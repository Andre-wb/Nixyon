use std::path::Path;

use nixyon_core::{BlockSegmenter, CancelFlag, EngineError, VoiceEmbedder, VoiceSpan, par_map};

use crate::blocks::blocks;
use crate::clip_spans::clip_spans;
use crate::config::SAMPLE_RATE;
use crate::read_wave::read_wave;
use crate::segment_embeddings::sample_range;
use crate::{SherpaError, SherpaModels};

/// Нейросегментация WAV 16 кГц блоками по `block_sec` секунд. Каждый отрезок говорящего
/// обрезается по кускам VAD `speech` и получает эмбеддинг нашей модели голоса; номера
/// говорящих sherpa отбрасываются, их назначает наша кластеризация по всей записи.
/// Отрезок, слишком короткий для модели голоса, выбрасывается. Итог по времени.
pub fn voice_spans(
    models: &SherpaModels,
    wav: &Path,
    speech: &[(f64, f64)],
    block_sec: f64,
    cancel: &CancelFlag,
) -> Result<Vec<VoiceSpan>, SherpaError> {
    let segmenter = models.segmenter().ok_or(SherpaError::NoSegmentationModel)?;
    let wave = read_wave(wav)?;
    let samples = wave.samples();
    let found = segment_speech(segmenter, samples, speech, block_sec, cancel)?;
    embed_spans(models.voice(), models.embed_jobs(), samples, &found, cancel)
}

/// Отрезки говорящих по нейросегментации записи 16 кГц блоками по `block_sec` секунд,
/// обрезанные по кускам VAD `speech`, без эмбеддингов.
pub fn segment_speech(
    segmenter: &dyn BlockSegmenter,
    samples: &[f32],
    speech: &[(f64, f64)],
    block_sec: f64,
    cancel: &CancelFlag,
) -> Result<Vec<(f64, f64)>, SherpaError> {
    if !block_sec.is_finite() || block_sec <= 0.0 {
        return Err(SherpaError::BadBlock(block_sec));
    }
    let rate = f64::from(SAMPLE_RATE);
    // Слишком большое значение насыщается при переводе в usize и обрезается длиной записи.
    let block = ((block_sec * rate).round() as usize).clamp(1, samples.len().max(1));

    let mut found: Vec<(f64, f64)> = Vec::new();
    for (a, b) in blocks(samples.len(), block) {
        if cancel.is_cancelled() {
            return Err(SherpaError::Cancelled);
        }
        let offset = a as f64 / rate;
        let end = b as f64 / rate;
        let spans = segmenter
            .speaker_spans(&samples[a..b])
            .map_err(|e| match e {
                EngineError::Cancelled => SherpaError::Cancelled,
                EngineError::Failed { message, .. } => SherpaError::SegmentationFailed {
                    offset,
                    reason: message,
                },
            })?;
        // Отрезок может чуть выйти за конец блока из-за шага кадров модели.
        found.extend(
            spans
                .into_iter()
                .map(|(s, e)| (offset + s, (offset + e).min(end))),
        );
    }
    Ok(clip_spans(&found, speech))
}

/// Эмбеддинги отрезков `spans` записи 16 кГц моделью `voice` в `jobs` потоков. Пустой
/// отрезок и отрезок, слишком короткий для модели голоса, выбрасываются; порядок прежний.
pub fn embed_spans(
    voice: &dyn VoiceEmbedder,
    jobs: usize,
    samples: &[f32],
    spans: &[(f64, f64)],
    cancel: &CancelFlag,
) -> Result<Vec<VoiceSpan>, SherpaError> {
    let found = par_map(spans, jobs, |&(start, end)| {
        if cancel.is_cancelled() {
            return Err(SherpaError::Cancelled);
        }
        let range = sample_range(start, end, samples.len());
        if range.is_empty() {
            return Ok(None);
        }
        let embedding = voice.embed(&samples[range]).map_err(SherpaError::from)?;
        Ok((!embedding.is_empty()).then_some(VoiceSpan {
            start,
            end,
            embedding,
        }))
    });
    found.into_iter().filter_map(Result::transpose).collect()
}
