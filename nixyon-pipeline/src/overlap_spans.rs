use crate::word_speakers::LabeledSpan;

/// Промежутки, где говорит кластер и одновременно с ним кто-то другой: пересечения
/// отрезков разных кластеров не короче `min_sec`. У одного кластера пересекающиеся и
/// смежные промежутки сливаются. Порядок: по кластеру, затем по началу.
pub(crate) fn overlap_spans(spans: &[LabeledSpan], min_sec: f64) -> Vec<LabeledSpan> {
    let mut parts: Vec<LabeledSpan> = Vec::new();
    for (i, &(a_start, a_end, a_label)) in spans.iter().enumerate() {
        for &(b_start, b_end, b_label) in &spans[i + 1..] {
            if a_label == b_label {
                continue;
            }
            let start = a_start.max(b_start);
            let end = a_end.min(b_end);
            if end - start >= min_sec && end > start {
                parts.push((start, end, a_label));
                parts.push((start, end, b_label));
            }
        }
    }
    parts.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.total_cmp(&b.0)));
    let mut merged: Vec<LabeledSpan> = Vec::with_capacity(parts.len());
    for part in parts {
        match merged.last_mut() {
            Some(last) if last.2 == part.2 && part.0 <= last.1 => last.1 = last.1.max(part.1),
            _ => merged.push(part),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_spans_of_two_voices_give_the_crossing_to_both() {
        let spans = [(0.0, 4.0, 0), (3.0, 6.0, 1)];
        assert_eq!(overlap_spans(&spans, 0.0), [(3.0, 4.0, 0), (3.0, 4.0, 1)]);
    }

    #[test]
    fn spans_of_one_voice_are_not_an_overlap() {
        let spans = [(0.0, 4.0, 0), (3.0, 6.0, 0)];
        assert!(overlap_spans(&spans, 0.0).is_empty());
    }

    #[test]
    fn touching_spans_are_not_an_overlap() {
        let spans = [(0.0, 3.0, 0), (3.0, 6.0, 1)];
        assert!(overlap_spans(&spans, 0.0).is_empty());
    }

    #[test]
    fn crossing_shorter_than_the_minimum_is_skipped() {
        let spans = [
            (0.0, 4.0, 0),
            (3.8, 6.0, 1),
            (10.0, 14.0, 0),
            (13.0, 16.0, 1),
        ];
        assert_eq!(
            overlap_spans(&spans, 0.3),
            [(13.0, 14.0, 0), (13.0, 14.0, 1)]
        );
    }

    #[test]
    fn crossings_of_one_voice_with_two_others_merge_into_one_span() {
        // Голос 0 говорит 0..10, голос 1 вступает в 2..5, голос 2 в 4..7.
        let spans = [(0.0, 10.0, 0), (2.0, 5.0, 1), (4.0, 7.0, 2)];
        assert_eq!(
            overlap_spans(&spans, 0.0),
            [(2.0, 7.0, 0), (2.0, 5.0, 1), (4.0, 7.0, 2)]
        );
    }
}
