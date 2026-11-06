use crate::SpeakerSpan;

/// Участок времени, на котором состав говорящих не меняется.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScoredInterval {
    pub duration: f64,
    /// Спикеры эталона, говорящие на участке, по возрастанию, без повторов.
    pub reference: Vec<u32>,
    pub hypothesis: Vec<u32>,
}

/// Режет время на участки по всем границам отрезков и выбрасывает воротник:
/// ± `collar` секунд вокруг каждой границы эталона не оцениваются, потому что
/// разметка на слух точнее не бывает.
pub(crate) fn scored_intervals(
    reference: &[SpeakerSpan],
    hypothesis: &[SpeakerSpan],
    collar: f64,
) -> Vec<ScoredInterval> {
    let unscored: Vec<(f64, f64)> = reference
        .iter()
        .flat_map(|s| [s.start, s.end])
        .map(|b| (b - collar, b + collar))
        .collect();
    let mut bounds: Vec<f64> = reference
        .iter()
        .chain(hypothesis)
        .flat_map(|s| [s.start, s.end])
        .chain(unscored.iter().flat_map(|&(a, b)| [a, b]))
        .collect();
    bounds.sort_by(f64::total_cmp);
    bounds.dedup();

    bounds
        .windows(2)
        .filter_map(|w| {
            let (a, b) = (w[0], w[1]);
            let mid = (a + b) / 2.0;
            if unscored.iter().any(|&(x, y)| x < mid && mid < y) {
                return None;
            }
            let reference = active(reference, mid);
            let hypothesis = active(hypothesis, mid);
            (!reference.is_empty() || !hypothesis.is_empty()).then_some(ScoredInterval {
                duration: b - a,
                reference,
                hypothesis,
            })
        })
        .collect()
}

fn active(spans: &[SpeakerSpan], t: f64) -> Vec<u32> {
    let mut speakers: Vec<u32> = spans
        .iter()
        .filter(|s| s.start < t && t < s.end)
        .map(|s| s.speaker)
        .collect();
    speakers.sort_unstable();
    speakers.dedup();
    speakers
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

    #[test]
    fn overlap_gives_both_speakers_on_the_shared_part() {
        let r = [span(1, 0.0, 2.0), span(2, 1.0, 3.0)];
        let got = scored_intervals(&r, &[], 0.0);
        let shape: Vec<(f64, Vec<u32>)> =
            got.into_iter().map(|i| (i.duration, i.reference)).collect();
        assert_eq!(shape, [(1.0, vec![1]), (1.0, vec![1, 2]), (1.0, vec![2])]);
    }

    #[test]
    fn collar_removes_time_around_reference_bounds_only() {
        let r = [span(1, 1.0, 3.0)];
        let h = [span(1, 1.5, 2.0)];
        let got = scored_intervals(&r, &h, 0.25);
        let total: f64 = got.iter().map(|i| i.duration).sum();
        assert_eq!(total, 1.5);
        assert!(got.iter().all(|i| i.reference == [1]));
    }

    #[test]
    fn silence_on_both_sides_is_not_an_interval() {
        let r = [span(1, 0.0, 1.0), span(1, 5.0, 6.0)];
        let total: f64 = scored_intervals(&r, &[], 0.0)
            .iter()
            .map(|i| i.duration)
            .sum();
        assert_eq!(total, 2.0);
    }

    #[test]
    fn repeated_spans_of_one_speaker_count_once() {
        let r = [span(1, 0.0, 2.0), span(1, 1.0, 2.0)];
        assert!(
            scored_intervals(&r, &[], 0.0)
                .iter()
                .all(|i| i.reference == [1])
        );
    }
}
