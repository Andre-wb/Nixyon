use std::collections::BTreeMap;

use crate::{PowersetWindow, overlap_probability};

/// Вероятность перекрытия по кадрам всей записи: кадры окон сводятся к общей сетке с шагом
/// кадра от середины первого кадра первого окна и усредняются по окнам. Хранит по числу
/// на кадр, поэтому окна можно добавлять по одному и не держать их в памяти.
#[derive(Debug, Clone, Default)]
pub struct OverlapGrid {
    origin: f64,
    step: f64,
    cells: BTreeMap<i64, (f32, u32)>,
}

impl OverlapGrid {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, window: &PowersetWindow) {
        if window.frame_sec <= 0.0 {
            return;
        }
        if self.step <= 0.0 {
            self.origin = window.first_frame_sec;
            self.step = window.frame_sec;
        }
        for (i, frame) in window.frames.iter().enumerate() {
            let cell = (window.frame_time(i) - self.origin) / self.step;
            let entry = self.cells.entry(cell.round() as i64).or_default();
            entry.0 += overlap_probability(frame);
            entry.1 += 1;
        }
    }

    /// Промежутки, где средняя вероятность перекрытия выше `threshold` в кадрах подряд;
    /// промежутки короче `min_sec` отбрасываются. По времени.
    pub fn spans(&self, threshold: f32, min_sec: f64) -> Vec<(f64, f64)> {
        let mut found: Vec<(f64, f64)> = Vec::new();
        let mut previous: Option<i64> = None;
        for (&cell, &(sum, count)) in &self.cells {
            if sum / count as f32 <= threshold {
                continue;
            }
            let start = self.origin + (cell as f64 - 0.5) * self.step;
            let end = self.origin + (cell as f64 + 0.5) * self.step;
            match found.last_mut() {
                Some(last) if previous == Some(cell - 1) => last.1 = end,
                _ => found.push((start, end)),
            }
            previous = Some(cell);
        }
        found.retain(|(start, end)| end - start >= min_sec);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: [f32; 7] = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    const TWO: [f32; 7] = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];

    fn window(first_frame_sec: f64, frames: Vec<[f32; 7]>) -> PowersetWindow {
        PowersetWindow {
            first_frame_sec,
            frame_sec: 0.5,
            frames,
        }
    }

    fn spans(windows: &[PowersetWindow], threshold: f32, min_sec: f64) -> Vec<(f64, f64)> {
        let mut grid = OverlapGrid::new();
        for w in windows {
            grid.add(w);
        }
        grid.spans(threshold, min_sec)
    }

    #[test]
    fn consecutive_overlap_frames_make_one_span() {
        let w = window(0.0, vec![ONE, TWO, TWO, ONE, TWO]);
        assert_eq!(spans(&[w], 0.5, 0.0), [(0.25, 1.25), (1.75, 2.25)]);
    }

    #[test]
    fn overlap_is_averaged_over_windows_that_see_the_frame() {
        // Кадр 1,0 с одно окно считает перекрытием, другое нет: в среднем 0,5.
        let a = window(0.0, vec![ONE, ONE, TWO]);
        let b = window(1.0, vec![ONE, ONE]);
        assert!(spans(&[a.clone(), b.clone()], 0.5, 0.0).is_empty());
        assert_eq!(spans(&[a, b], 0.4, 0.0), [(0.75, 1.25)]);
    }

    #[test]
    fn spans_shorter_than_the_minimum_are_dropped() {
        let w = window(0.0, vec![TWO, ONE, TWO, TWO]);
        assert_eq!(spans(&[w], 0.5, 0.6), [(0.75, 1.75)]);
    }

    #[test]
    fn grid_is_centred_on_the_frames() {
        // Середины кадров 0,5, 1,5, 2,5 с: перекрытие во втором кадре занимает 1..2 с.
        let w = PowersetWindow {
            first_frame_sec: 0.5,
            frame_sec: 1.0,
            frames: vec![ONE, TWO, ONE],
        };
        assert_eq!(spans(&[w], 0.5, 0.0), [(1.0, 2.0)]);
    }

    #[test]
    fn empty_grid_gives_no_spans() {
        assert!(OverlapGrid::new().spans(0.5, 0.0).is_empty());
    }
}
