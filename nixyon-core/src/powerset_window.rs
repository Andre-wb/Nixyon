use serde::{Deserialize, Serialize};

/// Число классов выхода pyannote segmentation-3.0: тишина, три говорящих поодиночке и
/// три пары.
pub const POWERSET_CLASSES: usize = 7;

/// Пара локальных говорящих окна у классов перекрытия, по порядку классов 4, 5, 6.
pub const POWERSET_PAIRS: [(usize, usize); 3] = [(0, 1), (0, 2), (1, 2)];

/// Индекс первого класса перекрытия.
pub const FIRST_OVERLAP_CLASS: usize = 4;

/// Одно окно нейросегментации pyannote: вероятности классов по кадрам. Номера говорящих
/// (0, 1, 2) свои у каждого окна и между окнами не совпадают.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowersetWindow {
    /// Секунды от начала записи до середины первого кадра.
    pub first_frame_sec: f64,
    /// Шаг кадров, секунды.
    pub frame_sec: f64,
    /// Вероятности классов каждого кадра, в сумме 1.
    pub frames: Vec<[f32; POWERSET_CLASSES]>,
}

impl PowersetWindow {
    /// Секунды от начала записи до середины кадра `index`.
    pub fn frame_time(&self, index: usize) -> f64 {
        self.first_frame_sec + index as f64 * self.frame_sec
    }
}

/// Вероятность того, что в кадре говорят двое.
pub fn overlap_probability(frame: &[f32; POWERSET_CLASSES]) -> f32 {
    frame[FIRST_OVERLAP_CLASS..].iter().sum()
}

/// Вероятность того, что локальный говорящий `speaker` (0, 1 или 2) звучит в кадре.
pub fn speaker_probability(frame: &[f32; POWERSET_CLASSES], speaker: usize) -> f32 {
    let alone = frame.get(1 + speaker).copied().unwrap_or(0.0);
    let paired: f32 = POWERSET_PAIRS
        .iter()
        .enumerate()
        .filter(|(_, (a, b))| *a == speaker || *b == speaker)
        .map(|(i, _)| frame[FIRST_OVERLAP_CLASS + i])
        .sum();
    alone + paired
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: [f32; POWERSET_CLASSES] = [0.1, 0.2, 0.05, 0.05, 0.3, 0.1, 0.2];

    #[test]
    fn overlap_probability_sums_the_pair_classes() {
        assert!((overlap_probability(&FRAME) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn speaker_probability_counts_the_speaker_alone_and_in_pairs() {
        assert!((speaker_probability(&FRAME, 0) - 0.6).abs() < 1e-6);
        assert!((speaker_probability(&FRAME, 1) - 0.55).abs() < 1e-6);
        assert!((speaker_probability(&FRAME, 2) - 0.35).abs() < 1e-6);
    }

    #[test]
    fn frame_time_steps_from_the_first_frame() {
        let w = PowersetWindow {
            first_frame_sec: 1.5,
            frame_sec: 0.25,
            frames: Vec::new(),
        };
        assert!((w.frame_time(4) - 2.5).abs() < 1e-9);
    }
}
