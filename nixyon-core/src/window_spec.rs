/// Окна для эмбеддингов голоса внутри куска речи: длина и шаг в секундах.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowSpec {
    window_sec: f64,
    step_sec: f64,
}

impl WindowSpec {
    /// Окно 1,5 с с шагом 0,5 с: на 1,5 с модель голоса уже даёт устойчивый эмбеддинг,
    /// а шаг 0,5 с задаёт точность границы до разреза по словам.
    pub const DEFAULT: Self = Self {
        window_sec: 1.5,
        step_sec: 0.5,
    };

    /// None, если длина или шаг не положительные или шаг длиннее окна:
    /// тогда между окнами оставались бы куски без эмбеддинга.
    pub fn new(window_sec: f64, step_sec: f64) -> Option<Self> {
        let ok = window_sec.is_finite()
            && step_sec.is_finite()
            && window_sec > 0.0
            && step_sec > 0.0
            && step_sec <= window_sec;
        ok.then_some(Self {
            window_sec,
            step_sec,
        })
    }

    pub fn window_sec(&self) -> f64 {
        self.window_sec
    }

    pub fn step_sec(&self) -> f64 {
        self.step_sec
    }

    /// Окна куска длиной `len` сэмплов: (начало, конец) в сэмплах от начала куска.
    /// Окна идут с шагом, последнее прижато к концу куска, чтобы конец не пропал.
    /// Кусок короче окна окон не получает: для него есть эмбеддинг всего куска.
    /// Та же формула в Kotlin (`WindowSpans.kt`), результаты должны совпадать.
    pub fn spans(&self, len: usize, rate: u32) -> Vec<(usize, usize)> {
        let samples = |sec: f64| (sec * f64::from(rate)).round() as usize;
        let window = samples(self.window_sec).max(1);
        let step = samples(self.step_sec).max(1);
        if len < window {
            return Vec::new();
        }
        let mut spans: Vec<(usize, usize)> = (0..=len - window)
            .step_by(step)
            .map(|start| (start, start + window))
            .collect();
        if spans.last().is_some_and(|&(_, end)| end < len) {
            spans.push((len - window, len));
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> WindowSpec {
        WindowSpec::new(1.5, 0.5).unwrap()
    }

    #[test]
    fn windows_step_through_the_segment_and_the_last_one_hugs_the_end() {
        // 2,7 с при 10 сэмплах в секунду: окна 15, шаг 5.
        assert_eq!(spec().spans(27, 10), [(0, 15), (5, 20), (10, 25), (12, 27)]);
    }

    #[test]
    fn exact_fit_adds_no_extra_window() {
        assert_eq!(spec().spans(25, 10), [(0, 15), (5, 20), (10, 25)]);
        assert_eq!(spec().spans(15, 10), [(0, 15)]);
    }

    #[test]
    fn segment_shorter_than_window_has_no_windows() {
        assert!(spec().spans(14, 10).is_empty());
        assert!(spec().spans(0, 10).is_empty());
    }

    #[test]
    fn real_rate_gives_whole_sample_windows() {
        let spans = spec().spans(16_000 * 3, 16_000);
        assert_eq!(spans.first(), Some(&(0, 24_000)));
        assert_eq!(spans.last(), Some(&(24_000, 48_000)));
        assert_eq!(spans.len(), 4);
    }

    #[test]
    fn bad_specs_are_refused() {
        assert_eq!(WindowSpec::new(0.0, 0.5), None);
        assert_eq!(WindowSpec::new(1.5, 0.0), None);
        assert_eq!(WindowSpec::new(1.0, 1.5), None);
        assert_eq!(WindowSpec::new(f64::NAN, 0.5), None);
        assert!(WindowSpec::new(1.5, 1.5).is_some());
    }
}
