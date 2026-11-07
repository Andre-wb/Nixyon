use std::path::Path;

use nixyon_core::{
    CancelFlag, EngineOptions, Pool, SegmentResult, SpeechEdges, hotwords_line, par_map,
};
use sherpa_onnx::OfflineRecognizer;

use crate::asr_groups::asr_groups;
use crate::config::{
    LONG_PIECE_SAMPLES, MAX_GAP_SAMPLES, MAX_GROUP_SAMPLES, SAMPLE_RATE, SPEECH_PAD_SAMPLES,
    WHOLE_IF_NO_SPEECH_SAMPLES,
};
use crate::long_pieces::split_long;
use crate::padded_bounds::padded_bounds;
use crate::read_wave::read_wave;
use crate::segment_embeddings::embed_segments;
use crate::speech_regions::speech_regions;
use crate::split_by_pieces::{PieceText, split_by_pieces};
use crate::{Decoding, SherpaError, SherpaModels};

/// По каждому куску речи WAV 16 кГц, найденному VAD: распознавание и эмбеддинг голоса,
/// а если запрошены окна, ещё эмбеддинги по окнам. В режиме `SpeechEdges::Padded` куски
/// берутся с запасом по краям, а короткая запись, где VAD речи не нашёл, идёт целиком.
/// Отмена проверяется перед каждым куском.
pub fn analyze(
    models: &SherpaModels,
    wav: &Path,
    options: &EngineOptions,
    cancel: &CancelFlag,
) -> Result<Vec<SegmentResult>, SherpaError> {
    let wave = read_wave(wav)?;
    let samples = wave.samples();
    let mut segments = recognize(models, samples, options, cancel)?;
    embed_segments(
        models.voice(),
        models.embed_jobs(),
        samples,
        &mut segments,
        options.windows,
        cancel,
    )?;
    Ok(segments)
}

/// Куски речи записи 16 кГц и их текст, без эмбеддингов голоса: эмбеддинги считает
/// `embed_segments` той моделью голоса, какая нужна.
pub fn recognize(
    models: &SherpaModels,
    samples: &[f32],
    options: &EngineOptions,
    cancel: &CancelFlag,
) -> Result<Vec<SegmentResult>, SherpaError> {
    let bounds = speech_bounds(models, samples, options)?;
    recognize_bounds(models, samples, &bounds, options, cancel)
}

/// Границы кусков речи в сэмплах: куски VAD (в режиме `SpeechEdges::Padded` с запасом по
/// краям, а короткая запись без речи целиком), без кусков, где вместо речи музыка. С
/// шумоподавителем (`SherpaModels::with_vad_denoiser`) длинные куски заменяются кусками
/// второго VAD по звуку после шумоподавления.
pub fn speech_bounds(
    models: &SherpaModels,
    samples: &[f32],
    options: &EngineOptions,
) -> Result<Vec<(usize, usize)>, SherpaError> {
    let music = match (options.drop_music, models.music_check()) {
        (false, _) => None,
        (true, Some(check)) => Some(check),
        (true, None) => return Err(SherpaError::NoAudioTagging),
    };
    let mut vad_config = models.vad_config_for(options.edges).clone();
    if let Some(sec) = options.max_speech_sec {
        vad_config.silero_vad.max_speech_duration = sec;
    }
    let vad = SherpaModels::new_vad(&vad_config)?;
    let padded = options.edges == SpeechEdges::Padded;
    let mut bounds: Vec<(usize, usize)> = speech_regions(&vad, samples)
        .iter()
        .map(|r| {
            let start = usize::try_from(r.start_sample).unwrap_or(0);
            (start, start + r.samples.len())
        })
        .collect();
    if let Some(denoiser) = models.vad_denoiser() {
        let clean = denoiser.clean_bounds(&vad_config, samples)?;
        bounds = split_long(&bounds, &clean, LONG_PIECE_SAMPLES);
    }
    if padded
        && bounds.is_empty()
        && !samples.is_empty()
        && samples.len() <= WHOLE_IF_NO_SPEECH_SAMPLES
    {
        bounds.push((0, samples.len()));
    }
    let pad = if padded { SPEECH_PAD_SAMPLES } else { 0 };
    Ok(padded_bounds(&bounds, samples.len(), pad)
        .into_iter()
        // Кусок с музыкой вместо речи не распознаётся: модель записала бы слова песни.
        .filter(|&(from, to)| music.is_none_or(|check| !check.is_music(&samples[from..to])))
        .collect())
}

/// Время куска в секундах по его границам в сэмплах, то же, что у распознанного куска.
pub fn bounds_seconds(from: usize, to: usize) -> (f64, f64) {
    let rate = f64::from(SAMPLE_RATE);
    // Номер сэмпла в пределах часов записи точно представим в f64.
    let start = from as f64 / rate;
    (start, start + (to - from) as f64 / rate)
}

/// Текст кусков речи по их границам; куски распознаются в столько потоков, сколько
/// распознавателей загружено, порядок прежний. Отмена проверяется перед каждым куском.
/// С `asr_groups` соседние куски с короткой паузой распознаются одним отрезком вместе с
/// паузой между ними, а слова раздаются обратно по кускам; куски и их границы прежние.
pub fn recognize_bounds(
    models: &SherpaModels,
    samples: &[f32],
    bounds: &[(usize, usize)],
    options: &EngineOptions,
    cancel: &CancelFlag,
) -> Result<Vec<SegmentResult>, SherpaError> {
    // Горячие слова есть только у распознавателя с поиском по лучам. Английский всегда
    // идёт жадным поиском (`Decoding::for_language`), его горячие слова не используются;
    // горячие слова при жадном поиске, выбранном при загрузке, остаются ошибкой.
    if !options.hotwords.is_empty() && models.decoding() == Decoding::Greedy {
        return Err(SherpaError::HotwordsNeedBeam);
    }
    let hotwords = match models.decoding().for_language(options.language) {
        Decoding::Greedy => String::new(),
        Decoding::HotwordBeam { .. } => hotwords_line(&options.hotwords),
    };
    let recognizers = models.recognizers().of(options.language)?;
    let recognizers = recognizers.as_ref();
    if !options.asr_groups {
        return par_map(bounds, models.asr_jobs(), |&(from, to)| {
            if cancel.is_cancelled() {
                return Err(SherpaError::Cancelled);
            }
            recognize_region(recognizers, from, to, &samples[from..to], &hotwords)
        })
        .into_iter()
        .collect();
    }
    let groups = asr_groups(bounds, MAX_GROUP_SAMPLES, MAX_GAP_SAMPLES);
    let parts = par_map(&groups, models.asr_jobs(), |group| {
        if cancel.is_cancelled() {
            return Err(SherpaError::Cancelled);
        }
        recognize_group(recognizers, samples, group, &hotwords)
    });
    let mut segments = Vec::with_capacity(bounds.len());
    for part in parts {
        segments.extend(part?);
    }
    Ok(segments)
}

// Группа распознаётся одним отрезком, слова раздаются по её кускам. Без времени токенов
// куски группы распознаются по одному.
fn recognize_group(
    recognizers: &Pool<OfflineRecognizer>,
    samples: &[f32],
    group: &[(usize, usize)],
    hotwords: &str,
) -> Result<Vec<SegmentResult>, SherpaError> {
    let (first, last) = (group[0].0, group[group.len() - 1].1);
    let whole = recognize_region(recognizers, first, last, &samples[first..last], hotwords)?;
    if group.len() == 1 {
        return Ok(vec![whole]);
    }
    let rate = f64::from(SAMPLE_RATE);
    let pieces: Vec<(f64, f64)> = group
        .iter()
        .map(|&(from, to)| ((from - first) as f64 / rate, (to - first) as f64 / rate))
        .collect();
    match split_by_pieces(&whole.tokens, &whole.token_times, &pieces) {
        Some(texts) => Ok(group
            .iter()
            .zip(texts)
            .map(|(&(from, to), piece)| piece_segment(from, to, piece))
            .collect()),
        None => group
            .iter()
            .map(|&(from, to)| {
                recognize_region(recognizers, from, to, &samples[from..to], hotwords)
            })
            .collect(),
    }
}

fn piece_segment(from: usize, to: usize, piece: PieceText) -> SegmentResult {
    let (start, end) = bounds_seconds(from, to);
    SegmentResult {
        start,
        end,
        text: piece.text,
        tokens: piece.tokens,
        token_times: piece.times,
        embedding: Vec::new(),
        windows: Vec::new(),
    }
}

fn recognize_region(
    recognizers: &Pool<OfflineRecognizer>,
    from: usize,
    to: usize,
    samples: &[f32],
    hotwords: &str,
) -> Result<SegmentResult, SherpaError> {
    let (start, end) = bounds_seconds(from, to);
    let result = recognizers
        .with(|recognizer| {
            let stream = if hotwords.is_empty() {
                recognizer.create_stream()
            } else {
                recognizer.create_stream_with_hotwords(hotwords)
            };
            stream.accept_waveform(SAMPLE_RATE, samples);
            recognizer.decode(&stream);
            stream.get_result()
        })
        .map_err(|_| SherpaError::Poisoned)?
        .ok_or(SherpaError::CreateFailed("результат распознавания"))?;

    Ok(SegmentResult {
        start,
        end,
        text: result.text,
        tokens: result.tokens,
        token_times: result.timestamps.unwrap_or_default(),
        embedding: Vec::new(),
        windows: Vec::new(),
    })
}

pub(crate) fn embed(models: &SherpaModels, samples: &[f32]) -> Result<Vec<f32>, SherpaError> {
    models.voice().embed(samples).map_err(SherpaError::from)
}
