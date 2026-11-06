use nixyon_core::{CancelFlag, SegmentResult, VoiceEmbedder, WindowEmbedding, WindowSpec, par_map};

use crate::SherpaError;
use crate::config::SAMPLE_RATE;

/// Эмбеддинги голоса кусков записи 16 кГц моделью `voice`, а если заданы окна, ещё по
/// окнам внутри куска. Куски считаются в `jobs` потоков, результат тот же, что по одному.
/// Прежние эмбеддинги кусков заменяются. Отмена проверяется перед каждым куском.
pub fn embed_segments(
    voice: &dyn VoiceEmbedder,
    jobs: usize,
    samples: &[f32],
    segments: &mut [SegmentResult],
    windows: Option<WindowSpec>,
    cancel: &CancelFlag,
) -> Result<(), SherpaError> {
    let found = par_map(segments, jobs, |segment| {
        if cancel.is_cancelled() {
            return Err(SherpaError::Cancelled);
        }
        let piece = &samples[sample_range(segment.start, segment.end, samples.len())];
        let embedding = voice.embed(piece).map_err(SherpaError::from)?;
        let windows = match windows {
            Some(spec) => window_embeddings(voice, piece, segment.start, spec)?,
            None => Vec::new(),
        };
        Ok((embedding, windows))
    });
    for (segment, result) in segments.iter_mut().zip(found) {
        (segment.embedding, segment.windows) = result?;
    }
    Ok(())
}

fn window_embeddings(
    voice: &dyn VoiceEmbedder,
    samples: &[f32],
    segment_start: f64,
    spec: WindowSpec,
) -> Result<Vec<WindowEmbedding>, SherpaError> {
    let rate = f64::from(SAMPLE_RATE);
    let mut windows = Vec::new();
    for (a, b) in spec.spans(samples.len(), SAMPLE_RATE.unsigned_abs()) {
        let embedding = voice.embed(&samples[a..b]).map_err(SherpaError::from)?;
        if !embedding.is_empty() {
            windows.push(WindowEmbedding {
                start: segment_start + a as f64 / rate,
                end: segment_start + b as f64 / rate,
                embedding,
            });
        }
    }
    Ok(windows)
}

/// Сэмплы куска по его времени. Начало куска получено делением номера сэмпла на частоту,
/// поэтому округление возвращает тот же номер.
pub(crate) fn sample_range(start: f64, end: f64, len: usize) -> std::ops::Range<usize> {
    let index = |sec: f64| ((sec * f64::from(SAMPLE_RATE)).round().max(0.0) as usize).min(len);
    let from = index(start);
    from..index(end).max(from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_range_returns_the_samples_the_segment_was_cut_from() {
        let rate = f64::from(SAMPLE_RATE);
        for (from, len) in [(0usize, 16_000usize), (123_457, 9_999), (86_399_999, 1)] {
            let start = from as f64 / rate;
            let end = start + len as f64 / rate;
            assert_eq!(sample_range(start, end, usize::MAX), from..from + len);
        }
        assert_eq!(sample_range(1.0, 2.0, 20_000), 16_000..20_000);
        assert_eq!(sample_range(2.0, 1.0, 40_000), 32_000..32_000);
    }
}
