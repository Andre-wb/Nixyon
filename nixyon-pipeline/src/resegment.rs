use nixyon_cluster::{cosine, unit, viterbi};
use nixyon_core::SegmentResult;

// Похожесть на говорящего, у которого нет центра: ниже любого косинуса.
const NO_CENTER: f64 = -2.0;

/// Итог пересегментации: кластер каждого куска и кластеры окон внутри него.
pub(crate) struct Resegmented {
    pub segment_labels: Vec<Option<usize>>,
    /// (начало, конец, кластер) в секундах от начала записи, по времени.
    pub window_labels: Vec<Vec<(f64, f64, usize)>>,
}

/// Пересегментация Витерби: окна всех кусков по времени (у куска без окон вместо них
/// весь кусок) получают метки с наименьшей ценой «непохожесть + штраф за смену».
/// Кусок получает метку, которую чаще всего получили его окна; при равенстве прежнюю.
/// Без центров или без единого голоса возвращает исходные метки.
pub(crate) fn resegment(
    segments: &[SegmentResult],
    cluster_labels: &[Option<usize>],
    centers: &[Option<Vec<f32>>],
    switch_penalty: f64,
) -> Resegmented {
    let mut owners: Vec<usize> = Vec::new();
    let mut spans: Vec<(f64, f64)> = Vec::new();
    let mut vectors: Vec<Vec<f32>> = Vec::new();
    for (i, s) in segments.iter().enumerate() {
        let before = owners.len();
        for w in &s.windows {
            if let Some(u) = unit(&w.embedding) {
                owners.push(i);
                spans.push((w.start, w.end));
                vectors.push(u);
            }
        }
        if owners.len() == before
            && let Some(u) = unit(&s.embedding)
        {
            owners.push(i);
            spans.push((s.start, s.end));
            vectors.push(u);
        }
    }
    let scores: Vec<Vec<f64>> = vectors
        .iter()
        .map(|u| {
            centers
                .iter()
                .map(|c| c.as_ref().map_or(NO_CENTER, |c| cosine(c, u)))
                .collect()
        })
        .collect();

    let labels = viterbi(&scores, switch_penalty);
    let mut window_labels: Vec<Vec<(f64, f64, usize)>> = vec![Vec::new(); segments.len()];
    if labels.len() != owners.len() {
        return Resegmented {
            segment_labels: cluster_labels.to_vec(),
            window_labels,
        };
    }
    for ((&owner, &(start, end)), &label) in owners.iter().zip(&spans).zip(&labels) {
        window_labels[owner].push((start, end, label));
    }
    let segment_labels = window_labels
        .iter()
        .zip(cluster_labels)
        .map(|(windows, &previous)| majority(windows, previous))
        .collect();
    Resegmented {
        segment_labels,
        window_labels,
    }
}

fn majority(windows: &[(f64, f64, usize)], previous: Option<usize>) -> Option<usize> {
    let mut counts: Vec<(usize, usize)> = Vec::new();
    for &(_, _, label) in windows {
        match counts.iter_mut().find(|(l, _)| *l == label) {
            Some((_, n)) => *n += 1,
            None => counts.push((label, 1)),
        }
    }
    let top = counts.iter().map(|&(_, n)| n).max()?;
    let tied: Vec<usize> = counts
        .iter()
        .filter(|&&(_, n)| n == top)
        .map(|&(l, _)| l)
        .collect();
    match previous {
        Some(p) if tied.contains(&p) => Some(p),
        _ => tied.into_iter().min(),
    }
}

#[cfg(test)]
mod tests {
    use nixyon_core::WindowEmbedding;

    use super::*;
    use crate::test_support::{segment, voice};

    fn window(start: f64, end: f64, embedding: Vec<f32>) -> WindowEmbedding {
        WindowEmbedding {
            start,
            end,
            embedding,
        }
    }

    fn centers() -> Vec<Option<Vec<f32>>> {
        vec![unit(&voice(1, 0.0)), unit(&voice(2, 0.0))]
    }

    #[test]
    fn segment_label_follows_the_majority_of_its_windows() {
        let mut s = segment(0.0, 3.0, "а", voice(1, 0.0));
        s.windows = vec![
            window(0.0, 1.5, voice(2, 0.0)),
            window(0.5, 2.0, voice(2, 0.0)),
            window(1.0, 2.5, voice(2, 0.0)),
            window(1.5, 3.0, voice(1, 0.0)),
        ];
        let r = resegment(&[s], &[Some(0)], &centers(), 0.0);
        assert_eq!(r.segment_labels, [Some(1)]);
        assert_eq!(
            r.window_labels[0].iter().map(|w| w.2).collect::<Vec<_>>(),
            [1, 1, 1, 0]
        );
    }

    #[test]
    fn tie_between_windows_keeps_the_previous_label() {
        let mut s = segment(0.0, 2.0, "а", voice(2, 0.0));
        s.windows = vec![
            window(0.0, 1.5, voice(1, 0.0)),
            window(0.5, 2.0, voice(2, 0.0)),
        ];
        let r = resegment(&[s], &[Some(1)], &centers(), 0.0);
        assert_eq!(r.segment_labels, [Some(1)]);
    }

    #[test]
    fn penalty_keeps_a_short_stray_segment_with_its_neighbours() {
        let segs = vec![
            segment(0.0, 2.0, "а", voice(1, 0.0)),
            segment(2.0, 2.6, "б", voice(2, 0.9)),
            segment(2.6, 5.0, "в", voice(1, 0.0)),
        ];
        let labels = [Some(0), Some(1), Some(0)];
        assert_eq!(
            resegment(&segs, &labels, &centers(), 0.0).segment_labels,
            labels
        );
        assert_eq!(
            resegment(&segs, &labels, &centers(), 0.5).segment_labels,
            [Some(0), Some(0), Some(0)]
        );
    }

    #[test]
    fn segment_without_voice_keeps_its_label_and_has_no_windows() {
        let segs = vec![
            segment(0.0, 2.0, "а", voice(1, 0.0)),
            segment(2.0, 2.2, "м", vec![]),
        ];
        let r = resegment(&segs, &[Some(0), None], &centers(), 0.2);
        assert_eq!(r.segment_labels, [Some(0), None]);
        assert!(r.window_labels[1].is_empty());
    }

    #[test]
    fn without_centers_labels_stay_as_they_were() {
        let segs = vec![segment(0.0, 2.0, "а", voice(1, 0.0))];
        let r = resegment(&segs, &[Some(3)], &[], 0.2);
        assert_eq!(r.segment_labels, [Some(3)]);
    }
}
