/// Пересечения отрезков нейросегментации с кусками VAD: распознавание идёт только
/// по кускам VAD, поэтому голос вне них не нужен. Отрезок, задевающий несколько
/// кусков, даёт часть в каждом. Пустые пересечения выбрасываются. Итог по времени.
pub(crate) fn clip_spans(spans: &[(f64, f64)], speech: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut clipped: Vec<(f64, f64)> = spans
        .iter()
        .flat_map(|&(a, b)| {
            speech
                .iter()
                .map(move |&(c, d)| (a.max(c), b.min(d)))
                .filter(|(start, end)| start < end)
        })
        .collect();
    clipped.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)));
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_is_cut_by_every_speech_piece_it_touches() {
        let speech = [(0.0, 2.0), (3.0, 5.0)];
        assert_eq!(clip_spans(&[(1.0, 4.0)], &speech), [(1.0, 2.0), (3.0, 4.0)]);
    }

    #[test]
    fn span_outside_speech_disappears() {
        assert!(clip_spans(&[(2.0, 3.0)], &[(0.0, 2.0), (3.0, 5.0)]).is_empty());
    }

    #[test]
    fn overlapping_spans_stay_overlapping_and_sorted() {
        let spans = [(1.5, 4.0), (0.5, 2.0)];
        assert_eq!(clip_spans(&spans, &[(0.0, 5.0)]), [(0.5, 2.0), (1.5, 4.0)]);
    }
}
