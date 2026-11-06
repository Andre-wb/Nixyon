use nixyon_cluster::min_cost_assignment;

use crate::scored_intervals::{ScoredInterval, scored_intervals};
use crate::{DerBreakdown, MetricsError, SpeakerSpan};

/// Воротник для DER: столько секунд вокруг каждой границы эталона не оценивается.
pub const DER_COLLAR_SEC: f64 = 0.25;

/// DER: номера спикеров гипотезы сопоставляются с эталоном один к одному так,
/// чтобы совпало больше всего времени; затем по каждому участку считаются пропуск,
/// ложная речь и путаница с учётом перекрытий.
pub fn diarization_errors(
    reference: &[SpeakerSpan],
    hypothesis: &[SpeakerSpan],
    collar: f64,
) -> Result<DerBreakdown, MetricsError> {
    if !collar.is_finite() || collar < 0.0 {
        return Err(MetricsError::BadCollar(collar));
    }
    check_spans(reference)?;
    check_spans(hypothesis)?;

    let intervals = scored_intervals(reference, hypothesis, collar);
    let reference_ids = speaker_ids(reference);
    let hypothesis_ids = speaker_ids(hypothesis);
    let mapping = best_mapping(&intervals, &reference_ids, &hypothesis_ids);

    let mut d = DerBreakdown::default();
    for interval in &intervals {
        // Число спикеров на участке мало, перевод в f64 точный.
        let n_ref = interval.reference.len() as f64;
        let n_hyp = interval.hypothesis.len() as f64;
        let correct = interval
            .hypothesis
            .iter()
            .filter(|h| {
                mapping
                    .iter()
                    .any(|&(r, mapped)| mapped == **h && interval.reference.contains(&r))
            })
            .count() as f64;
        d.reference_speech += n_ref * interval.duration;
        d.missed += (n_ref - n_hyp).max(0.0) * interval.duration;
        d.false_alarm += (n_hyp - n_ref).max(0.0) * interval.duration;
        d.confusion += (n_ref.min(n_hyp) - correct) * interval.duration;
    }
    Ok(d)
}

pub(crate) fn check_spans(spans: &[SpeakerSpan]) -> Result<(), MetricsError> {
    match spans
        .iter()
        .find(|s| !s.start.is_finite() || !s.end.is_finite() || s.end < s.start)
    {
        Some(s) => Err(MetricsError::BadSpan {
            speaker: s.speaker,
            start: s.start,
            end: s.end,
        }),
        None => Ok(()),
    }
}

fn speaker_ids(spans: &[SpeakerSpan]) -> Vec<u32> {
    let mut ids: Vec<u32> = spans.iter().map(|s| s.speaker).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Пары (спикер эталона, спикер гипотезы) с наибольшим общим временем.
fn best_mapping(
    intervals: &[ScoredInterval],
    reference_ids: &[u32],
    hypothesis_ids: &[u32],
) -> Vec<(u32, u32)> {
    let n = reference_ids.len().max(hypothesis_ids.len());
    let mut cost = vec![vec![0.0; n]; n];
    for interval in intervals {
        for r in &interval.reference {
            for h in &interval.hypothesis {
                if let (Ok(ri), Ok(hi)) = (
                    reference_ids.binary_search(r),
                    hypothesis_ids.binary_search(h),
                ) {
                    cost[ri][hi] -= interval.duration;
                }
            }
        }
    }
    min_cost_assignment(&cost)
        .into_iter()
        .enumerate()
        .filter_map(|(ri, hi)| Some((*reference_ids.get(ri)?, *hypothesis_ids.get(hi)?)))
        .collect()
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

    fn der(r: &[SpeakerSpan], h: &[SpeakerSpan], collar: f64) -> DerBreakdown {
        diarization_errors(r, h, collar).unwrap()
    }

    #[test]
    fn same_segmentation_with_other_numbers_has_zero_error() {
        let r = [span(1, 0.0, 2.0), span(2, 2.0, 5.0), span(1, 5.0, 6.0)];
        let h = [span(7, 0.0, 2.0), span(3, 2.0, 5.0), span(7, 5.0, 6.0)];
        let d = der(&r, &h, 0.0);
        assert_eq!(d.rate(), Some(0.0));
        assert_eq!(d.reference_speech, 6.0);
    }

    #[test]
    fn whole_speaker_given_to_another_is_confusion() {
        let r = [span(1, 0.0, 4.0), span(2, 4.0, 6.0)];
        let h = [span(1, 0.0, 6.0)];
        let d = der(&r, &h, 0.0);
        assert_eq!(d.confusion, 2.0);
        assert_eq!(d.missed + d.false_alarm, 0.0);
        assert_eq!(d.rate(), Some(2.0 / 6.0));
    }

    #[test]
    fn missed_and_false_speech_are_counted_separately() {
        let r = [span(1, 0.0, 4.0)];
        let h = [span(1, 1.0, 6.0)];
        let d = der(&r, &h, 0.0);
        assert_eq!(d.missed, 1.0);
        assert_eq!(d.false_alarm, 2.0);
        assert_eq!(d.confusion, 0.0);
    }

    #[test]
    fn overlap_heard_as_one_speaker_is_a_miss_of_the_other() {
        let r = [span(1, 0.0, 4.0), span(2, 2.0, 6.0)];
        let h = [span(1, 0.0, 4.0), span(2, 4.0, 6.0)];
        let d = der(&r, &h, 0.0);
        assert_eq!(d.reference_speech, 8.0);
        assert_eq!(d.missed, 2.0);
        assert_eq!(d.confusion, 0.0);
    }

    #[test]
    fn mapping_maximizes_total_shared_time_not_first_match() {
        // Жадное сопоставление по первому отрезку дало бы пару 1 и 10, хотя у 10 больше общего с 2.
        let r = [span(1, 0.0, 1.0), span(2, 1.0, 10.0)];
        let h = [span(10, 0.0, 10.0), span(20, 0.0, 0.5)];
        let d = der(&r, &h, 0.0);
        assert_eq!(d.confusion, 1.0 - 0.5);
        assert_eq!(d.false_alarm, 0.5);
    }

    #[test]
    fn boundary_shift_inside_collar_is_not_an_error() {
        let r = [span(1, 0.0, 2.0), span(2, 2.0, 4.0)];
        let h = [span(1, 0.0, 2.2), span(2, 2.2, 4.0)];
        assert_eq!(der(&r, &h, DER_COLLAR_SEC).rate(), Some(0.0));
        assert!(der(&r, &h, 0.0).confusion > 0.0);
    }

    #[test]
    fn empty_hypothesis_misses_everything() {
        let r = [span(1, 0.0, 3.0)];
        assert_eq!(der(&r, &[], 0.0).rate(), Some(1.0));
    }

    #[test]
    fn broken_input_is_rejected() {
        let ok = [span(1, 0.0, 1.0)];
        assert_eq!(
            diarization_errors(&[span(3, 2.0, 1.0)], &ok, 0.0),
            Err(MetricsError::BadSpan {
                speaker: 3,
                start: 2.0,
                end: 1.0
            })
        );
        assert!(diarization_errors(&ok, &[span(1, f64::NAN, 1.0)], 0.0).is_err());
        assert_eq!(
            diarization_errors(&ok, &ok, -0.1),
            Err(MetricsError::BadCollar(-0.1))
        );
    }
}
