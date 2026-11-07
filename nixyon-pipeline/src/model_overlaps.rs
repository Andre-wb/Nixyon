use std::collections::BTreeMap;

use nixyon_core::{PowersetWindow, overlap_probability};

/// Промежутки, где по нейросегментации говорят двое: кадры всех окон сводятся к общей
/// сетке с шагом кадра от середины первого кадра, вероятность перекрытия усредняется по окнам, кадры выше `threshold`
/// подряд дают промежуток. Промежутки короче `min_sec` отбрасываются. По времени.
pub(crate) fn model_overlaps(
    windows: &[PowersetWindow],
    threshold: f32,
    min_sec: f64,
) -> Vec<(f64, f64)> {
    let Some(first) = windows.first().filter(|w| w.frame_sec > 0.0) else {
        return Vec::new();
    };
    let (origin, step) = (first.first_frame_sec, first.frame_sec);
    let mut grid: BTreeMap<i64, (f32, u32)> = BTreeMap::new();
    for w in windows {
        for (i, frame) in w.frames.iter().enumerate() {
            let cell = grid
                .entry(cell_of(w.frame_time(i) - origin, step))
                .or_default();
            cell.0 += overlap_probability(frame);
            cell.1 += 1;
        }
    }
    let mut found: Vec<(f64, f64)> = Vec::new();
    let mut previous: Option<i64> = None;
    for (&cell, &(sum, count)) in &grid {
        if sum / count as f32 <= threshold {
            continue;
        }
        let start = origin + (cell as f64 - 0.5) * step;
        let end = origin + (cell as f64 + 0.5) * step;
        match found.last_mut() {
            Some(last) if previous == Some(cell - 1) => last.1 = end,
            _ => found.push((start, end)),
        }
        previous = Some(cell);
    }
    found.retain(|(start, end)| end - start >= min_sec);
    found
}

fn cell_of(time: f64, step: f64) -> i64 {
    (time / step).round() as i64
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

    #[test]
    fn consecutive_overlap_frames_make_one_span() {
        let w = window(0.0, vec![ONE, TWO, TWO, ONE, TWO]);
        assert_eq!(model_overlaps(&[w], 0.5, 0.0), [(0.25, 1.25), (1.75, 2.25)]);
    }

    #[test]
    fn overlap_is_averaged_over_windows_that_see_the_frame() {
        // Кадр 1,0 с одно окно считает перекрытием, другое нет: в среднем 0,5.
        let a = window(0.0, vec![ONE, ONE, TWO]);
        let b = window(1.0, vec![ONE, ONE]);
        assert!(model_overlaps(&[a.clone(), b.clone()], 0.5, 0.0).is_empty());
        assert_eq!(model_overlaps(&[a, b], 0.4, 0.0), [(0.75, 1.25)]);
    }

    #[test]
    fn spans_shorter_than_the_minimum_are_dropped() {
        let w = window(0.0, vec![TWO, ONE, TWO, TWO]);
        assert_eq!(model_overlaps(&[w], 0.5, 0.6), [(0.75, 1.75)]);
    }

    #[test]
    fn grid_is_centred_on_the_frames() {
        // Середины кадров 0,5, 1,5, 2,5 с: перекрытие во втором кадре занимает 1..2 с.
        let w = PowersetWindow {
            first_frame_sec: 0.5,
            frame_sec: 1.0,
            frames: vec![ONE, TWO, ONE],
        };
        assert_eq!(model_overlaps(&[w], 0.5, 0.0), [(1.0, 2.0)]);
    }

    #[test]
    fn no_windows_give_no_spans() {
        assert!(model_overlaps(&[], 0.5, 0.0).is_empty());
    }
}
