use std::path::Path;
use std::time::Instant;

use nixyon_core::{
    BlockSegmenter, CancelFlag, EngineOptions, SegmentResult, VoiceEmbedder, VoiceSpan,
};
use nixyon_ort::{OrtError, OrtManifest};
use nixyon_sherpa::{
    SherpaError, SherpaModels, bounds_seconds, embed_segments, embed_spans, read_samples,
    recognize_bounds, segment_speech, speech_bounds,
};

use crate::{Combo, Segmenter, VoiceModel};

/// Модели голоса и нейросегментации сочетаний сверх основного набора: каждая модель
/// загружается один раз, даже если её берут несколько сочетаний.
pub struct ComboModels {
    voices: Vec<(VoiceModel, Box<dyn VoiceEmbedder>)>,
    segmenters: Vec<(Segmenter, Box<dyn BlockSegmenter>)>,
}

impl ComboModels {
    /// `ort`: набор ort, нужен, если хоть одно сочетание берёт модель из него. `threads`:
    /// потоки на вызов модели, `jobs`: сессий модели голоса для параллельных эмбеддингов.
    pub fn load(
        combos: &[Combo],
        ort: Option<&OrtManifest>,
        threads: usize,
        jobs: usize,
    ) -> Result<Self, OrtError> {
        let mut voices: Vec<(VoiceModel, Box<dyn VoiceEmbedder>)> = Vec::new();
        let mut segmenters: Vec<(Segmenter, Box<dyn BlockSegmenter>)> = Vec::new();
        for combo in combos {
            let needs = combo.voice.needs_ort() || combo.segmenter.needs_ort();
            let ort = match ort {
                Some(ort) => ort,
                None if needs => return Err(OrtError::Runtime("нет набора ort".into())),
                None => continue,
            };
            if !voices.iter().any(|(m, _)| *m == combo.voice)
                && let Some(voice) = combo.voice.load(ort, threads, jobs)?
            {
                voices.push((combo.voice, voice));
            }
            if !segmenters.iter().any(|(s, _)| *s == combo.segmenter)
                && let Some(segmenter) = combo.segmenter.load(ort, threads)?
            {
                segmenters.push((combo.segmenter, segmenter));
            }
        }
        Ok(Self { voices, segmenters })
    }

    fn voice<'a>(&'a self, base: &'a SherpaModels, model: VoiceModel) -> &'a dyn VoiceEmbedder {
        self.voices
            .iter()
            .find(|(m, _)| *m == model)
            .map_or(base.voice(), |(_, v)| v.as_ref())
    }

    fn segmenter<'a>(
        &'a self,
        base: &'a SherpaModels,
        kind: Segmenter,
    ) -> Option<&'a dyn BlockSegmenter> {
        match self.segmenters.iter().find(|(s, _)| *s == kind) {
            Some((_, s)) => Some(s.as_ref()),
            None => base.segmenter(),
        }
    }
}

/// Сырые отрезки одной нейросегментации, ещё без эмбеддингов.
type RawSpans = (Segmenter, Vec<(f64, f64)>);

/// Итог одного сочетания по записи: куски с эмбеддингами его модели голоса, отрезки его
/// нейросегментации и время, потраченное только на это сочетание.
pub struct ComboAnalysis {
    pub segments: Vec<SegmentResult>,
    pub spans: Vec<VoiceSpan>,
    pub own_sec: f64,
}

/// Распознаёт запись один раз (VAD и текст), затем на каждое сочетание считает свои
/// эмбеддинги кусков и отрезков. Сырые отрезки одной нейросегментации считаются один раз
/// и делятся между сочетаниями; распознавание и нейросегментации идут одновременно. `neural_block_sec`: длина блока нейросегментации, None без
/// нейросегментации. Итоги в порядке сочетаний.
pub fn analyze_combos(
    base: &SherpaModels,
    extra: &ComboModels,
    combos: &[Combo],
    wav_16k: &Path,
    engine: &EngineOptions,
    neural_block_sec: Option<f64>,
    cancel: &CancelFlag,
) -> Result<Vec<ComboAnalysis>, SherpaError> {
    let samples = read_samples(wav_16k)?;
    let bounds = speech_bounds(base, &samples, engine)?;
    let speech: Vec<(f64, f64)> = bounds.iter().map(|&(a, b)| bounds_seconds(a, b)).collect();
    let mut kinds: Vec<Segmenter> = Vec::new();
    if neural_block_sec.is_some() {
        for combo in combos {
            if !kinds.contains(&combo.segmenter) {
                kinds.push(combo.segmenter);
            }
        }
    }
    let segmenters = kinds
        .iter()
        .map(|&kind| {
            extra
                .segmenter(base, kind)
                .map(|s| (kind, s))
                .ok_or(SherpaError::NoSegmentationModel)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Распознавание и каждая нейросегментация идут одновременно, каждая в своём потоке:
    // сегментации нужны только границы речи, а не текст.
    let (recognized, raw) = std::thread::scope(|s| {
        let asr = s.spawn(|| recognize_bounds(base, &samples, &bounds, engine, cancel));
        let found: Vec<_> = segmenters
            .iter()
            .map(|&(kind, segmenter)| {
                let samples = &samples;
                let speech = &speech;
                s.spawn(move || {
                    let block_sec = neural_block_sec.unwrap_or_default();
                    segment_speech(segmenter, samples, speech, block_sec, cancel)
                        .map(|spans| (kind, spans))
                })
            })
            .collect();
        let recognized: Result<Vec<SegmentResult>, SherpaError> = joined(asr);
        let raw: Result<Vec<RawSpans>, SherpaError> = found.into_iter().map(joined).collect();
        (recognized, raw)
    });
    let recognized = recognized?;
    let raw = raw?;
    let mut results = Vec::with_capacity(combos.len());
    for combo in combos {
        let own = Instant::now();
        let voice = extra.voice(base, combo.voice);
        let mut segments = recognized.clone();
        embed_segments(
            voice,
            base.embed_jobs(),
            &samples,
            &mut segments,
            engine.windows,
            cancel,
        )?;
        let spans = match raw.iter().find(|(s, _)| *s == combo.segmenter) {
            Some((_, found)) => embed_spans(voice, base.embed_jobs(), &samples, found, cancel)?,
            None => Vec::new(),
        };
        results.push(ComboAnalysis {
            segments,
            spans,
            own_sec: own.elapsed().as_secs_f64(),
        });
    }
    Ok(results)
}

// Итог потока; паника потока продолжается в вызывающем.
fn joined<T>(handle: std::thread::ScopedJoinHandle<'_, T>) -> T {
    match handle.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
