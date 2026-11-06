use std::collections::HashMap;

use nixyon_metrics::{WordEdit, word_alignment_relaxed};
use serde::Serialize;

use crate::ClipResult;

/// Сколько самых частых замен попадает в отчёт.
pub const TOP_SUBSTITUTIONS: usize = 20;

/// Замена слова эталона другим словом и сколько раз она встретилась.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubstitutionCount {
    pub reference: String,
    pub hypothesis: String,
    pub count: usize,
}

/// Самые частые замены слов по всем клипам: чаще раньше, при равенстве по алфавиту.
/// Запись не считается: «десять» и «10», «щас» и «сейчас» не замены.
pub fn top_substitutions(results: &[ClipResult], limit: usize) -> Vec<SubstitutionCount> {
    let mut counts: HashMap<(String, String), usize> = HashMap::new();
    for clip in results {
        for edit in word_alignment_relaxed(&clip.reference, &clip.hypothesis) {
            if let WordEdit::Replaced {
                reference,
                hypothesis,
            } = edit
            {
                *counts.entry((reference, hypothesis)).or_insert(0) += 1;
            }
        }
    }
    let mut sorted: Vec<SubstitutionCount> = counts
        .into_iter()
        .map(|((reference, hypothesis), count)| SubstitutionCount {
            reference,
            hypothesis,
            count,
        })
        .collect();
    sorted.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.reference.cmp(&b.reference))
            .then_with(|| a.hypothesis.cmp(&b.hypothesis))
    });
    sorted.truncate(limit);
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClipHypothesis;

    fn clip(reference: &str, heard: &str) -> ClipResult {
        ClipResult::new(
            "a",
            reference,
            1.0,
            0.1,
            Ok(ClipHypothesis {
                text: heard.into(),
                speakers: 1,
            }),
        )
    }

    #[test]
    fn substitutions_are_counted_over_clips_most_frequent_first() {
        let results = [
            clip("Я иду домой.", "я еду домой"),
            clip("иду быстро", "еду быстро"),
            clip("кот спит", "код спит лишнее"),
            clip("всё верно", "все верно"),
        ];
        let top = top_substitutions(&results, 10);
        assert_eq!(
            top,
            [
                SubstitutionCount {
                    reference: "иду".into(),
                    hypothesis: "еду".into(),
                    count: 2
                },
                SubstitutionCount {
                    reference: "кот".into(),
                    hypothesis: "код".into(),
                    count: 1
                },
            ]
        );
        assert_eq!(top_substitutions(&results, 1).len(), 1);
    }

    #[test]
    fn missed_words_of_an_empty_answer_are_not_substitutions() {
        assert!(top_substitutions(&[clip("раз два", "")], 10).is_empty());
    }
}
