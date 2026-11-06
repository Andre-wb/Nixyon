use crate::level::{dbfs, rms};

/// Окно, по которому меряется уровень.
pub const WINDOW_SEC: f64 = 0.1;
/// Уровень, к которому подтягивается речь.
pub const TARGET_DBFS: f32 = -20.0;
/// Усиление ограничено, чтобы не раздувать шум и не давить громкую речь в ноль.
pub const MAX_GAIN_DB: f32 = 30.0;
pub const MIN_GAIN_DB: f32 = -10.0;
/// Уровень речи берётся по самому громкому окну в этой окрестности: так усиление
/// не дёргается внутри фразы, но успевает смениться в паузе между людьми.
const NEIGHBOURHOOD_SEC: f64 = 1.5;
/// Окно считается речью, если громче шума записи хотя бы на столько.
const SPEECH_ABOVE_NOISE_DB: f32 = 10.0;
const SPEECH_MIN_DBFS: f32 = -65.0;
/// Доля самых тихих окон, по которой оценивается шум записи. Маленькая: если взять
/// больше, при коротких паузах шумом окажется тихая речь, и её не усилят.
const NOISE_PERCENTILE: f64 = 0.02;
const PEAK_LIMIT: f32 = 0.99;

/// Первый проход: уровень и пик каждого окна. Сэмплы не хранятся.
pub struct LevelAnalyzer {
    window_len: usize,
    window: Vec<f32>,
    rms: Vec<f32>,
    peak: Vec<f32>,
}

impl LevelAnalyzer {
    pub fn new(sample_rate: u32) -> Self {
        let window_len = ((f64::from(sample_rate) * WINDOW_SEC) as usize).max(1);
        Self {
            window_len,
            window: Vec::with_capacity(window_len),
            rms: Vec::new(),
            peak: Vec::new(),
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        let mut rest = samples;
        while !rest.is_empty() {
            let take = (self.window_len - self.window.len()).min(rest.len());
            self.window.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.window.len() == self.window_len {
                self.close_window();
            }
        }
    }

    pub fn finish(mut self) -> GainCurve {
        if !self.window.is_empty() {
            self.close_window();
        }
        GainCurve::new(self.window_len, &self.rms, &self.peak)
    }

    fn close_window(&mut self) {
        self.rms.push(rms(&self.window));
        self.peak
            .push(self.window.iter().fold(0.0f32, |m, s| m.max(s.abs())));
        self.window.clear();
    }
}

/// Усиление по окнам. Между центрами окон усиление меняется линейно, без ступенек.
pub struct GainCurve {
    window_len: usize,
    gains: Vec<f32>,
}

impl GainCurve {
    fn new(window_len: usize, rms_per_window: &[f32], peak_per_window: &[f32]) -> Self {
        let levels: Vec<f32> = rms_per_window.iter().map(|&r| dbfs(r)).collect();
        let speech_floor = (noise_floor(&levels) + SPEECH_ABOVE_NOISE_DB).max(SPEECH_MIN_DBFS);
        let reach = ((NEIGHBOURHOOD_SEC / WINDOW_SEC).round() as usize).max(1);
        let gains = (0..levels.len())
            .map(|i| {
                let from = i.saturating_sub(reach);
                let to = (i + reach + 1).min(levels.len());
                let local = levels[from..to]
                    .iter()
                    .copied()
                    .fold(f32::NEG_INFINITY, f32::max);
                if local < speech_floor {
                    return 1.0;
                }
                let gain_db = (TARGET_DBFS - local).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
                let local_peak = peak_per_window[from..to]
                    .iter()
                    .copied()
                    .fold(0.0f32, f32::max);
                let gain = 10f32.powf(gain_db / 20.0);
                if local_peak * gain > PEAK_LIMIT && local_peak > 0.0 {
                    (PEAK_LIMIT / local_peak).max(10f32.powf(MIN_GAIN_DB / 20.0))
                } else {
                    gain
                }
            })
            .collect();
        Self { window_len, gains }
    }

    /// Усиление для сэмпла с номером `index` от начала записи.
    pub fn gain_at(&self, index: u64) -> f32 {
        let Some(&last) = self.gains.last() else {
            return 1.0;
        };
        let half = self.window_len as f64 / 2.0;
        let position = (index as f64 - half) / self.window_len as f64;
        if position <= 0.0 {
            return self.gains[0];
        }
        let left = position.floor() as usize;
        if left + 1 >= self.gains.len() {
            return last;
        }
        let frac = (position - left as f64) as f32;
        self.gains[left] * (1.0 - frac) + self.gains[left + 1] * frac
    }
}

/// Второй проход: применяет усиление к потоку сэмплов по порядку.
pub struct GainApplier<'a> {
    curve: &'a GainCurve,
    position: u64,
}

impl<'a> GainApplier<'a> {
    pub fn new(curve: &'a GainCurve) -> Self {
        Self { curve, position: 0 }
    }

    pub fn apply(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = (*s * self.curve.gain_at(self.position)).clamp(-1.0, 1.0);
            self.position += 1;
        }
    }
}

fn noise_floor(levels: &[f32]) -> f32 {
    if levels.is_empty() {
        return SPEECH_MIN_DBFS;
    }
    let mut sorted = levels.to_vec();
    sorted.sort_by(f32::total_cmp);
    let index = ((sorted.len() - 1) as f64 * NOISE_PERCENTILE).round() as usize;
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sine;

    fn normalize(samples: &[f32]) -> Vec<f32> {
        let mut analyzer = LevelAnalyzer::new(16_000);
        for chunk in samples.chunks(999) {
            analyzer.push(chunk);
        }
        let curve = analyzer.finish();
        let mut out = samples.to_vec();
        let mut applier = GainApplier::new(&curve);
        for chunk in out.chunks_mut(777) {
            applier.apply(chunk);
        }
        out
    }

    fn level(samples: &[f32]) -> f32 {
        dbfs(rms(samples))
    }

    fn with_noise_floor(mut samples: Vec<f32>) -> Vec<f32> {
        // Слабый шум -70 дБ, как у настоящей записи; детерминированный.
        let mut state = 1u32;
        for s in &mut samples {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *s += ((state >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.001;
        }
        samples
    }

    #[test]
    fn quiet_speaker_after_loud_one_is_brought_to_the_same_level() {
        let mut samples = sine(16_000, 300.0, 0.5, 3.0);
        samples.extend(vec![0.0; 8_000]);
        samples.extend(sine(16_000, 200.0, 0.01, 3.0));
        let out = normalize(&with_noise_floor(samples));
        let loud = level(&out[8_000..40_000]);
        let quiet = level(&out[64_000..96_000]);
        assert!(
            (loud - quiet).abs() < 3.0,
            "громкий {loud:.1} дБ, тихий {quiet:.1} дБ"
        );
        assert!(
            (quiet - (TARGET_DBFS - 3.0)).abs() < 3.0,
            "тихий {quiet:.1} дБ"
        );
    }

    #[test]
    fn long_silence_far_from_speech_is_not_amplified() {
        let mut samples = sine(16_000, 300.0, 0.3, 2.0);
        samples.extend(vec![0.0; 16_000 * 10]);
        let samples = with_noise_floor(samples);
        let out = normalize(&samples);
        let middle = 16_000 * 7..16_000 * 9;
        assert!((level(&out[middle.clone()]) - level(&samples[middle])).abs() < 0.5);
    }

    #[test]
    fn amplified_signal_never_exceeds_peak_limit() {
        // Пики выше среднего уровня: без ограничения усиление вывело бы их за 1.
        let mut samples = sine(16_000, 300.0, 0.02, 2.0);
        for s in samples.iter_mut().step_by(400) {
            *s = 0.9;
        }
        let out = normalize(&with_noise_floor(samples));
        assert!(out.iter().all(|s| s.abs() <= PEAK_LIMIT + 1e-6));
    }

    #[test]
    fn gain_is_clamped_to_maximum() {
        let samples = with_noise_floor(sine(16_000, 300.0, 0.0005, 2.0));
        let out = normalize(&samples);
        let gained = level(&out[8_000..24_000]) - level(&samples[8_000..24_000]);
        assert!(gained <= MAX_GAIN_DB + 0.5, "усиление {gained:.1} дБ");
    }

    #[test]
    fn gain_changes_smoothly_between_windows() {
        let mut samples = sine(16_000, 300.0, 0.5, 2.0);
        samples.extend(sine(16_000, 300.0, 0.01, 2.0));
        let mut analyzer = LevelAnalyzer::new(16_000);
        analyzer.push(&with_noise_floor(samples));
        let curve = analyzer.finish();
        let max_step = (1..64_000u64)
            .map(|i| (curve.gain_at(i) - curve.gain_at(i - 1)).abs())
            .fold(0.0f32, f32::max);
        assert!(max_step < 0.05, "скачок усиления {max_step}");
    }

    #[test]
    fn empty_input_is_left_alone() {
        assert!(normalize(&[]).is_empty());
        let curve = LevelAnalyzer::new(16_000).finish();
        assert_eq!(curve.gain_at(123), 1.0);
    }
}
