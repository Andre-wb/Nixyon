use nixyon_cluster::unit;

use crate::SpeakerCenter;

/// Центры после слияния спикеров: центр `absorb` вливается в центр `keep` со средним,
/// взвешенным по времени речи, и пропадает. Если центра `keep` нет, им становится центр
/// `absorb`. Итог снова единичный вектор.
pub fn merge_centers(
    centers: &mut Vec<SpeakerCenter>,
    keep: u32,
    absorb: u32,
    keep_weight: f64,
    absorb_weight: f64,
) {
    let Some(at) = centers.iter().position(|(id, _)| *id == absorb) else {
        return;
    };
    let (_, absorbed) = centers.remove(at);
    match centers.iter_mut().find(|(id, _)| *id == keep) {
        Some((_, kept)) => {
            let (a, b) = (keep_weight.max(0.0), absorb_weight.max(0.0));
            // Без времени речи у обоих центры весят поровну.
            let (a, b) = if a + b > 0.0 { (a, b) } else { (1.0, 1.0) };
            let mixed: Vec<f32> = kept
                .iter()
                .zip(&absorbed)
                .map(|(x, y)| ((f64::from(*x) * a + f64::from(*y) * b) / (a + b)) as f32)
                .collect();
            if let Some(u) = unit(&mixed) {
                *kept = u;
            }
        }
        None => centers.push((keep, absorbed)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorbed_center_is_mixed_in_by_talk_time_and_disappears() {
        let mut centers = vec![
            (1, vec![1.0, 0.0]),
            (2, vec![0.0, 1.0]),
            (3, vec![1.0, 0.0]),
        ];
        merge_centers(&mut centers, 1, 2, 3.0, 1.0);
        assert_eq!(centers.len(), 2);
        let (id, kept) = &centers[0];
        assert_eq!(*id, 1);
        // (0,75, 0,25) после нормировки.
        assert!((kept[0] - 0.948_683).abs() < 1e-5 && (kept[1] - 0.316_228).abs() < 1e-5);
    }

    #[test]
    fn without_talk_time_centers_weigh_equally() {
        let mut centers = vec![(1, vec![1.0, 0.0]), (2, vec![0.0, 1.0])];
        merge_centers(&mut centers, 1, 2, 0.0, 0.0);
        assert!((centers[0].1[0] - centers[0].1[1]).abs() < 1e-6);
    }

    #[test]
    fn missing_centers_are_handled() {
        let mut centers = vec![(2, vec![0.0, 1.0])];
        merge_centers(&mut centers, 1, 2, 1.0, 1.0);
        assert_eq!(centers, vec![(1, vec![0.0, 1.0])]);
        merge_centers(&mut centers, 1, 9, 1.0, 1.0);
        assert_eq!(centers, vec![(1, vec![0.0, 1.0])]);
    }
}
