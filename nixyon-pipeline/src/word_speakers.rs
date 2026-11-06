use nixyon_core::Word;

/// Говорящий у слова и признак того, что в этот момент говорят несколько человек.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WordSpeaker {
    pub speaker: usize,
    pub overlap: bool,
}

/// Отрезок нейросегментации с кластером: начало, конец, кластер.
pub(crate) type LabeledSpan = (f64, f64, usize);

/// Говорящий каждого слова по середине слова. Нет отрезков в этой точке: `fallback`.
/// Один кластер: он. Несколько: перекрытие, слово остаётся у говорящего предыдущего
/// слова, если он среди звучащих, иначе у того, чей отрезок начался раньше
/// (его перебили). Распознавание даёт один поток слов, поэтому слово не дублируется.
pub(crate) fn word_speakers(
    words: &[Word],
    spans: &[LabeledSpan],
    fallback: usize,
) -> Vec<WordSpeaker> {
    let mut result: Vec<WordSpeaker> = Vec::with_capacity(words.len());
    for w in words {
        let mid = (w.start + w.end) / 2.0;
        let mut active: Vec<LabeledSpan> = spans
            .iter()
            .copied()
            .filter(|&(start, end, _)| start <= mid && mid < end)
            .collect();
        active.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
        let mut labels: Vec<usize> = active.iter().map(|s| s.2).collect();
        labels.sort_unstable();
        labels.dedup();
        let previous = result.last().map(|p| p.speaker);
        let speaker = match labels.as_slice() {
            [] => WordSpeaker {
                speaker: fallback,
                overlap: false,
            },
            [one] => WordSpeaker {
                speaker: *one,
                overlap: false,
            },
            _ => WordSpeaker {
                speaker: previous
                    .filter(|p| labels.contains(p))
                    .unwrap_or(active[0].2),
                overlap: true,
            },
        };
        result.push(speaker);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::word;

    fn speakers(words: &[Word], spans: &[LabeledSpan], fallback: usize) -> Vec<(usize, bool)> {
        word_speakers(words, spans, fallback)
            .into_iter()
            .map(|w| (w.speaker, w.overlap))
            .collect()
    }

    #[test]
    fn word_belongs_to_the_span_under_its_middle() {
        // Слово 0,8..1,6 начинается в отрезке 0, а середина 1,2 уже в отрезке 1.
        let words = [word("да", 0.8, 1.6)];
        let spans = [(0.0, 1.0, 0), (1.0, 3.0, 1)];
        assert_eq!(speakers(&words, &spans, 9), [(1, false)]);
    }

    #[test]
    fn word_outside_spans_takes_the_fallback() {
        let words = [word("а", 5.0, 6.0)];
        assert_eq!(speakers(&words, &[(0.0, 1.0, 0)], 7), [(7, false)]);
    }

    #[test]
    fn overlap_keeps_the_previous_speaker_and_is_marked() {
        let words = [
            word("а", 0.0, 1.0),
            word("б", 1.0, 2.0),
            word("в", 2.0, 3.0),
        ];
        // Говорящий 1 вступает в 1,2: слово «б» в перекрытии остаётся у 0,
        // потому что предыдущее слово у 0.
        let spans = [(0.0, 2.5, 0), (1.2, 3.0, 1)];
        assert_eq!(
            speakers(&words, &spans, 9),
            [(0, false), (0, true), (1, false)]
        );
    }

    #[test]
    fn overlap_without_a_previous_word_goes_to_the_earlier_span() {
        let words = [word("а", 1.0, 2.0)];
        let spans = [(0.5, 3.0, 1), (0.0, 2.0, 0), (1.0, 2.0, 2)];
        assert_eq!(speakers(&words, &spans, 9), [(0, true)]);
        let later_first = [(0.0, 3.0, 1), (0.5, 2.0, 0)];
        assert_eq!(speakers(&words, &later_first, 9), [(1, true)]);
    }

    #[test]
    fn two_spans_of_one_cluster_are_not_an_overlap() {
        let words = [word("а", 1.0, 2.0)];
        assert_eq!(
            speakers(&words, &[(0.0, 3.0, 4), (1.0, 2.5, 4)], 9),
            [(4, false)]
        );
    }
}
