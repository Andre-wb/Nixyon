use crate::level::{dbfs, rms};

/// Запись короче этого не обрабатывается.
pub const MIN_DURATION_SEC: f64 = 1.0;
/// Если даже самое громкое окно тише этого порога, в записи ничего не слышно.
pub const SILENCE_DBFS: f32 = -50.0;
/// Сэмпл с модулем от этого значения считается упёршимся в предел.
pub const CLIP_LEVEL: f32 = 0.999;
/// Доля таких сэмплов, после которой запись помечается как перегруженная.
pub const CLIP_RATIO: f64 = 0.001;
const WINDOW_SEC: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityIssue {
    TooShort,
    Silent,
    Clipping,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityReport {
    pub duration_sec: f64,
    /// Уровень самого громкого окна 0,1 с.
    pub loudest_window_dbfs: f32,
    pub clipped_ratio: f64,
}

impl QualityReport {
    /// TooShort и Silent означают, что обрабатывать нечего; Clipping только предупреждение.
    pub fn issues(&self) -> Vec<QualityIssue> {
        let mut issues = Vec::new();
        if self.duration_sec < MIN_DURATION_SEC {
            issues.push(QualityIssue::TooShort);
        }
        if self.loudest_window_dbfs < SILENCE_DBFS {
            issues.push(QualityIssue::Silent);
        }
        if self.clipped_ratio > CLIP_RATIO {
            issues.push(QualityIssue::Clipping);
        }
        issues
    }
}

/// Считает качество записи потоком, по мере поступления сэмплов.
pub struct QualityMeter {
    sample_rate: u32,
    window_len: usize,
    window: Vec<f32>,
    frames: u64,
    clipped: u64,
    loudest: f32,
}

impl QualityMeter {
    pub fn new(sample_rate: u32) -> Self {
        // Окно 0,1 с: при частоте до 192 кГц это не больше 19 200 сэмплов.
        let window_len = ((f64::from(sample_rate) * WINDOW_SEC) as usize).max(1);
        Self {
            sample_rate,
            window_len,
            window: Vec::with_capacity(window_len),
            frames: 0,
            clipped: 0,
            loudest: 0.0,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.frames += samples.len() as u64;
        self.clipped += samples.iter().filter(|s| s.abs() >= CLIP_LEVEL).count() as u64;
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

    pub fn finish(mut self) -> QualityReport {
        if !self.window.is_empty() {
            self.close_window();
        }
        QualityReport {
            duration_sec: self.frames as f64 / f64::from(self.sample_rate),
            loudest_window_dbfs: dbfs(self.loudest),
            clipped_ratio: if self.frames == 0 {
                0.0
            } else {
                self.clipped as f64 / self.frames as f64
            },
        }
    }

    fn close_window(&mut self) {
        self.loudest = self.loudest.max(rms(&self.window));
        self.window.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sine;

    fn report(samples: &[f32]) -> QualityReport {
        let mut meter = QualityMeter::new(16_000);
        for chunk in samples.chunks(777) {
            meter.push(chunk);
        }
        meter.finish()
    }

    #[test]
    fn normal_speech_level_has_no_issues() {
        let r = report(&sine(16_000, 440.0, 0.3, 2.0));
        assert!((r.duration_sec - 2.0).abs() < 1e-9);
        assert!(r.issues().is_empty(), "{r:?}");
    }

    #[test]
    fn shorter_than_one_second_is_too_short() {
        let r = report(&sine(16_000, 440.0, 0.3, 0.999));
        assert_eq!(r.issues(), [QualityIssue::TooShort]);
        let exactly = report(&sine(16_000, 440.0, 0.3, 1.0));
        assert!(exactly.issues().is_empty());
    }

    #[test]
    fn zeros_and_very_quiet_signal_are_silent() {
        assert_eq!(report(&[0.0; 32_000]).issues(), [QualityIssue::Silent]);
        // −60 дБ: амплитуда синуса 0,001 × √2, RMS 0,001.
        let quiet = sine(16_000, 440.0, 0.001 * 2f32.sqrt(), 2.0);
        assert_eq!(report(&quiet).issues(), [QualityIssue::Silent]);
    }

    #[test]
    fn short_loud_burst_in_long_silence_is_not_silent() {
        let mut samples = vec![0.0; 16_000 * 10];
        samples.extend(sine(16_000, 440.0, 0.3, 0.2));
        assert!(report(&samples).issues().is_empty());
    }

    #[test]
    fn clipping_above_ratio_is_reported_and_below_is_not() {
        let mut samples = sine(16_000, 440.0, 0.3, 2.0);
        // 0,1 % от 32 000 это ровно 32 сэмпла: на пороге ещё не перегруз.
        for s in samples.iter_mut().take(32) {
            *s = 1.0;
        }
        assert!(report(&samples).issues().is_empty());
        samples[32] = -1.0;
        assert_eq!(report(&samples).issues(), [QualityIssue::Clipping]);
    }

    #[test]
    fn empty_input_is_too_short_and_silent() {
        assert_eq!(
            report(&[]).issues(),
            [QualityIssue::TooShort, QualityIssue::Silent]
        );
    }
}
