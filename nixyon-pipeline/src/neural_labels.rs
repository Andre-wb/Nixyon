use nixyon_cluster::{nearest, unit};
use nixyon_core::{SegmentResult, VoiceSpan};

use crate::speaker_labels::{cluster_units, fill_from_time_neighbours};
use crate::{PipelineOptions, SpeakerCount};

/// Кусок VAD, который отрезки нейросегментации покрывают меньше чем на эту долю,
/// кластеризуется сам, своим эмбеддингом: сегментация иногда пропускает голос целиком,
/// а VAD его слышит.
pub(crate) const MIN_SPAN_COVERAGE: f64 = 0.5;

/// Кластеры отрезков нейросегментации и кусков VAD.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NeuralLabels {
    /// Кластер каждого отрезка; None, если у отрезка нет голоса.
    pub spans: Vec<Option<usize>>,
    /// Кластер каждого куска: свой, если кусок кластеризовался сам; иначе тот,
    /// которому принадлежит больше всего времени отрезков внутри куска; иначе ближайший
    /// центр к эмбеддингу куска; иначе кластер соседа по времени.
    pub segments: Vec<Option<usize>>,
    /// Центры кластеров по номеру кластера; у пустого кластера центра нет.
    pub centers: Vec<Option<Vec<f32>>>,
}

/// Общая кластеризация по всей записи: отрезки нейросегментации и плохо покрытые ими
/// куски VAD. Куски идут по времени.
pub(crate) fn neural_labels(
    segments: &[SegmentResult],
    spans: &[VoiceSpan],
    count: SpeakerCount,
    options: &PipelineOptions,
) -> NeuralLabels {
    let own: Vec<bool> = segments
        .iter()
        .map(|s| coverage(s.start, s.end, spans) < MIN_SPAN_COVERAGE)
        .collect();
    let mut durations: Vec<f64> = spans.iter().map(|s| s.end - s.start).collect();
    let mut units: Vec<Option<Vec<f32>>> = spans.iter().map(|s| unit(&s.embedding)).collect();
    for (s, &own) in segments.iter().zip(&own) {
        durations.push(s.end - s.start);
        units.push(if own { unit(&s.embedding) } else { None });
    }
    let (labels, centers) = cluster_units(&durations, &units, count, options);
    let (span_labels, own_labels) = labels.split_at(spans.len());

    let mut segment_labels: Vec<Option<usize>> = segments
        .iter()
        .zip(own_labels)
        .map(|(s, &own_label)| {
            own_label
                .or_else(|| dominant(s.start, s.end, spans, span_labels))
                .or_else(|| unit(&s.embedding).and_then(|u| nearest(&centers, &u)))
        })
        .collect();
    fill_from_time_neighbours(&mut segment_labels);
    NeuralLabels {
        spans: span_labels.to_vec(),
        segments: segment_labels,
        centers,
    }
}

/// Доля времени куска, где есть хотя бы один отрезок.
pub(crate) fn coverage(start: f64, end: f64, spans: &[VoiceSpan]) -> f64 {
    let mut parts: Vec<(f64, f64)> = spans
        .iter()
        .map(|s| (s.start.max(start), s.end.min(end)))
        .filter(|(a, b)| a < b)
        .collect();
    parts.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut covered = 0.0;
    let mut reached = start;
    for (a, b) in parts {
        let from = a.max(reached);
        if b > from {
            covered += b - from;
            reached = b;
        }
    }
    let length = end - start;
    if length > 0.0 { covered / length } else { 0.0 }
}

// Кластер, которому внутри куска принадлежит больше всего времени отрезков;
// при равенстве меньший номер.
fn dominant(start: f64, end: f64, spans: &[VoiceSpan], labels: &[Option<usize>]) -> Option<usize> {
    let mut time: Vec<(usize, f64)> = Vec::new();
    for (s, label) in spans.iter().zip(labels) {
        let overlap = s.end.min(end) - s.start.max(start);
        if let Some(l) = label
            && overlap > 0.0
        {
            match time.iter_mut().find(|(x, _)| x == l) {
                Some((_, t)) => *t += overlap,
                None => time.push((*l, overlap)),
            }
        }
    }
    time.into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(l, _)| l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{segment, voice};

    fn span(start: f64, end: f64, embedding: Vec<f32>) -> VoiceSpan {
        VoiceSpan {
            start,
            end,
            embedding,
        }
    }

    #[test]
    fn segment_covered_by_spans_takes_the_voice_that_holds_most_of_its_time() {
        // Внутри куска 0..6 два голоса, у первого 4 с, у второго 2 с.
        let segs = vec![segment(0.0, 6.0, "а", voice(2, 0.0))];
        let spans = vec![
            span(0.0, 4.0, voice(1, 0.0)),
            span(4.0, 6.0, voice(2, 0.0)),
            span(7.0, 9.0, voice(2, 0.1)),
        ];
        let l = neural_labels(
            &segs,
            &spans,
            SpeakerCount::Known(2),
            &PipelineOptions::default(),
        );
        assert_ne!(l.spans[0], l.spans[1]);
        assert_eq!(l.spans[1], l.spans[2]);
        assert_eq!(l.segments, [l.spans[0]]);
    }

    #[test]
    fn segment_missed_by_segmentation_is_clustered_by_its_own_voice() {
        // Третий голос сегментация не нашла: кусок 4..8 покрыт на 0,25 и даёт свой кластер.
        let segs = vec![
            segment(0.0, 3.0, "а", voice(1, 0.0)),
            segment(4.0, 8.0, "б", voice(3, 0.0)),
            segment(9.0, 12.0, "в", voice(2, 0.0)),
        ];
        let spans = vec![
            span(0.0, 3.0, voice(1, 0.0)),
            span(4.0, 5.0, voice(1, 0.05)),
            span(9.0, 12.0, voice(2, 0.0)),
        ];
        let l = neural_labels(
            &segs,
            &spans,
            SpeakerCount::Known(3),
            &PipelineOptions::default(),
        );
        let ids: Vec<usize> = l.segments.iter().map(|x| x.unwrap()).collect();
        assert_ne!(ids[1], ids[0]);
        assert_ne!(ids[1], ids[2]);
        assert_ne!(ids[0], ids[2]);
    }

    #[test]
    fn without_spans_segments_are_clustered_as_before() {
        let segs = vec![
            segment(0.0, 2.0, "а", voice(1, 0.0)),
            segment(2.0, 4.0, "б", voice(2, 0.0)),
            segment(4.0, 6.0, "в", voice(1, 0.1)),
        ];
        let l = neural_labels(
            &segs,
            &[],
            SpeakerCount::Known(2),
            &PipelineOptions::default(),
        );
        assert_eq!(l.segments[0], l.segments[2]);
        assert_ne!(l.segments[0], l.segments[1]);
    }

    #[test]
    fn coverage_counts_overlapping_spans_once() {
        let spans = vec![
            span(0.0, 3.0, vec![]),
            span(2.0, 4.0, vec![]),
            span(9.0, 11.0, vec![]),
        ];
        assert!((coverage(0.0, 10.0, &spans) - 0.5).abs() < 1e-12);
        assert_eq!(coverage(5.0, 5.0, &spans), 0.0);
    }
}
