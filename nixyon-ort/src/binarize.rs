use crate::median::median_filter;

/// Перевод вероятностей речи говорящих по кадрам в отрезки: медианный фильтр, затем
/// порог с гистерезисом. Отрезок начинается, когда вероятность доходит до `onset`, и
/// кончается, когда опускается ниже `offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Binarize {
    pub onset: f32,
    pub offset: f32,
    /// Окно медианного фильтра в кадрах, нечётное; 1 выключает фильтр.
    pub median_frames: usize,
}

impl Default for Binarize {
    /// Как `diarize()` в NeMo для Sortformer: порог 0,5 без сглаживания.
    fn default() -> Self {
        Self {
            onset: 0.5,
            offset: 0.5,
            median_frames: 1,
        }
    }
}

impl Binarize {
    /// `probs`: кадры подряд, в каждом по `speakers` вероятностей. Итог: (начало, конец)
    /// в секундах, по говорящим, внутри говорящего по времени.
    pub fn spans(&self, probs: &[f32], speakers: usize, frame_sec: f64) -> Vec<(f64, f64)> {
        if speakers == 0 {
            return Vec::new();
        }
        let frames = probs.len() / speakers;
        let mut spans = Vec::new();
        for speaker in 0..speakers {
            let column: Vec<f32> = (0..frames).map(|f| probs[f * speakers + speaker]).collect();
            let smooth = median_filter(&column, self.median_frames);
            let mut start = None;
            for (f, &p) in smooth.iter().enumerate() {
                match start {
                    None if p >= self.onset => start = Some(f),
                    Some(s) if p < self.offset => {
                        spans.push((s as f64 * frame_sec, f as f64 * frame_sec));
                        start = None;
                    }
                    _ => {}
                }
            }
            if let Some(s) = start {
                spans.push((s as f64 * frame_sec, frames as f64 * frame_sec));
            }
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_threshold_gives_one_span_per_run_of_speech() {
        // Два говорящих, кадр 0,1 с: первый говорит в кадрах 1-2, второй в кадре 3 до конца.
        let probs = [0.1, 0.0, 0.9, 0.0, 0.8, 0.2, 0.2, 0.7];
        let spans = Binarize::default().spans(&probs, 2, 0.1);
        assert_eq!(spans.len(), 2);
        assert!((spans[0].0 - 0.1).abs() < 1e-9 && (spans[0].1 - 0.3).abs() < 1e-9);
        assert!((spans[1].0 - 0.3).abs() < 1e-9 && (spans[1].1 - 0.4).abs() < 1e-9);
    }

    #[test]
    fn hysteresis_keeps_a_span_through_a_dip_above_offset() {
        let b = Binarize {
            onset: 0.6,
            offset: 0.3,
            median_frames: 1,
        };
        assert_eq!(b.spans(&[0.7, 0.4, 0.7, 0.1], 1, 1.0), [(0.0, 3.0)]);
    }

    #[test]
    fn median_filter_drops_a_one_frame_blip() {
        let b = Binarize {
            median_frames: 3,
            ..Binarize::default()
        };
        assert!(b.spans(&[0.0, 0.0, 0.9, 0.0, 0.0], 1, 1.0).is_empty());
    }

    #[test]
    fn no_speakers_give_no_spans() {
        assert!(Binarize::default().spans(&[0.9], 0, 1.0).is_empty());
    }
}
