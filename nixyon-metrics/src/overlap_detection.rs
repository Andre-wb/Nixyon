use crate::der::check_spans;
use crate::{MetricsError, OverlapScore, SpeakerSpan};

/// Точность и полнота пометки перекрытий по времени, без воротника: перекрытия
/// бывают короче полсекунды, воротник съел бы их целиком. Перекрытие эталона: время,
/// где говорят хотя бы два разных спикера. `found`: помеченные отрезки (начало, конец),
/// наложения между ними считаются один раз.
pub fn overlap_detection(
    reference: &[SpeakerSpan],
    found: &[(f64, f64)],
) -> Result<OverlapScore, MetricsError> {
    check_spans(reference)?;
    if let Some(&(start, end)) = found
        .iter()
        .find(|(a, b)| !a.is_finite() || !b.is_finite() || b < a)
    {
        return Err(MetricsError::BadInterval { start, end });
    }
    let truth = reference_overlaps(reference);
    let marked = union(found.to_vec());
    Ok(OverlapScore {
        reference_sec: length(&truth),
        found_sec: length(&marked),
        // fold с 0,0, а не sum: сумма пустого набора f64 даёт -0,0, и в таблице
        // появляется «-0,0 %».
        hit_sec: truth
            .iter()
            .flat_map(|&(a, b)| {
                marked
                    .iter()
                    .map(move |&(c, d)| (b.min(d) - a.max(c)).max(0.0))
            })
            .fold(0.0, |acc, x| acc + x),
    })
}

// Участки между соседними границами, на которых звучат хотя бы два разных спикера.
fn reference_overlaps(spans: &[SpeakerSpan]) -> Vec<(f64, f64)> {
    let mut bounds: Vec<f64> = spans.iter().flat_map(|s| [s.start, s.end]).collect();
    bounds.sort_by(f64::total_cmp);
    bounds.dedup();
    let pieces = bounds
        .windows(2)
        .filter(|w| {
            let mid = (w[0] + w[1]) / 2.0;
            let mut speakers: Vec<u32> = spans
                .iter()
                .filter(|s| s.start < mid && mid < s.end)
                .map(|s| s.speaker)
                .collect();
            speakers.sort_unstable();
            speakers.dedup();
            speakers.len() >= 2
        })
        .map(|w| (w[0], w[1]))
        .collect();
    union(pieces)
}

fn union(mut parts: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    parts.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(parts.len());
    for (a, b) in parts {
        match merged.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => merged.push((a, b)),
        }
    }
    merged
}

fn length(parts: &[(f64, f64)]) -> f64 {
    parts.iter().fold(0.0, |acc, (a, b)| acc + (b - a))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(speaker: u32, start: f64, end: f64) -> SpeakerSpan {
        SpeakerSpan {
            speaker,
            start,
            end,
        }
    }

    // Перекрытия 2..3 и 6..6,5.
    fn reference() -> Vec<SpeakerSpan> {
        vec![span(1, 0.0, 3.0), span(2, 2.0, 6.5), span(1, 6.0, 9.0)]
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn exact_marks_give_full_precision_and_recall() {
        let s = overlap_detection(&reference(), &[(2.0, 3.0), (6.0, 6.5)]).unwrap();
        assert!(close(s.reference_sec, 1.5));
        assert_eq!((s.precision(), s.recall()), (Some(1.0), Some(1.0)));
    }

    #[test]
    fn mark_wider_than_the_overlap_loses_precision_not_recall() {
        let s = overlap_detection(&reference(), &[(1.0, 3.0), (6.0, 6.5)]).unwrap();
        assert!(close(s.found_sec, 2.5));
        assert!(close(s.precision().unwrap(), 1.5 / 2.5));
        assert_eq!(s.recall(), Some(1.0));
    }

    #[test]
    fn missed_overlap_loses_recall() {
        let s = overlap_detection(&reference(), &[(2.0, 3.0)]).unwrap();
        assert_eq!(s.precision(), Some(1.0));
        assert!(close(s.recall().unwrap(), 1.0 / 1.5));
    }

    #[test]
    fn nothing_marked_has_no_precision_and_zero_recall() {
        let s = overlap_detection(&reference(), &[]).unwrap();
        assert_eq!((s.precision(), s.recall()), (None, Some(0.0)));
        assert!(
            s.recall().unwrap().is_sign_positive(),
            "ноль без знака минус"
        );
        assert!(s.found_sec.is_sign_positive());
    }

    #[test]
    fn mark_where_one_speaker_talks_is_a_false_mark() {
        let s = overlap_detection(&[span(1, 0.0, 5.0)], &[(1.0, 2.0)]).unwrap();
        assert_eq!((s.precision(), s.recall()), (Some(0.0), None));
    }

    #[test]
    fn one_speaker_overlapping_himself_is_not_an_overlap() {
        let s = overlap_detection(&[span(1, 0.0, 3.0), span(1, 2.0, 4.0)], &[]).unwrap();
        assert_eq!(s.reference_sec, 0.0);
    }

    #[test]
    fn three_speakers_at_once_count_their_time_once() {
        let r = [span(1, 0.0, 4.0), span(2, 1.0, 4.0), span(3, 2.0, 4.0)];
        let s = overlap_detection(&r, &[(1.0, 3.0), (2.0, 4.0)]).unwrap();
        assert!(close(s.reference_sec, 3.0));
        assert!(close(s.found_sec, 3.0));
        assert_eq!(s.recall(), Some(1.0));
    }

    #[test]
    fn broken_intervals_are_errors() {
        assert!(matches!(
            overlap_detection(&reference(), &[(3.0, 2.0)]),
            Err(MetricsError::BadInterval { .. })
        ));
        assert!(matches!(
            overlap_detection(&[span(1, 1.0, f64::NAN)], &[]),
            Err(MetricsError::BadSpan { .. })
        ));
    }
}
