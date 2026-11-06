use nixyon_metrics::WordEdit;
use serde::Serialize;

use crate::ClipResult;

/// Метка клипа для фильтра на странице проверки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipTag {
    /// Есть ошибки по словам.
    Errors,
    /// Текст получен, но пустой.
    Empty,
    /// Текст не получен: ошибка декодера или движка.
    Failed,
    /// Пропущено первое слово эталона (и, может быть, следующие).
    CutStart,
    /// Пропущено последнее слово эталона (и, может быть, предыдущие).
    CutEnd,
    /// Модель записала число цифрами.
    Digits,
}

/// Метки клипа по итогу и выравниванию слов. Пустой ответ и ошибка не считаются
/// обрывами: там пропущены все слова.
pub fn clip_tags(clip: &ClipResult, edits: &[WordEdit]) -> Vec<ClipTag> {
    let mut tags = Vec::new();
    if clip.word_errors > 0 {
        tags.push(ClipTag::Errors);
    }
    if clip.failure.is_some() {
        tags.push(ClipTag::Failed);
    } else if clip.is_empty() {
        tags.push(ClipTag::Empty);
    } else {
        // Лишние слова эталона не касаются, по краям смотрим только слова эталона.
        let mut on_reference = edits.iter().filter(|e| !matches!(e, WordEdit::Extra(_)));
        if matches!(on_reference.next(), Some(WordEdit::Missed(_))) {
            tags.push(ClipTag::CutStart);
        }
        if matches!(on_reference.next_back(), Some(WordEdit::Missed(_))) {
            tags.push(ClipTag::CutEnd);
        }
    }
    if clip.has_digits() {
        tags.push(ClipTag::Digits);
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClipHypothesis;
    use nixyon_metrics::word_alignment;

    fn tags(reference: &str, heard: &str) -> Vec<ClipTag> {
        let clip = ClipResult::new(
            "a",
            reference,
            1.0,
            0.1,
            Ok(ClipHypothesis {
                text: heard.into(),
                speakers: 1,
            }),
        );
        clip_tags(&clip, &word_alignment(reference, heard))
    }

    #[test]
    fn missed_words_at_the_edges_are_cuts() {
        assert_eq!(
            tags("он стоял не снимая пальто", "стоял не снимая"),
            [ClipTag::Errors, ClipTag::CutStart, ClipTag::CutEnd]
        );
        assert_eq!(
            tags("раз два три", "раз два"),
            [ClipTag::Errors, ClipTag::CutEnd]
        );
        assert_eq!(
            tags("раз два три", "ну два три"),
            [ClipTag::Errors],
            "замена, а не обрыв"
        );
        assert_eq!(
            tags("раз два три", "ну раз два"),
            [ClipTag::Errors, ClipTag::CutEnd],
            "лишнее слово в начале не мешает"
        );
    }

    #[test]
    fn empty_answer_and_failure_are_not_cuts() {
        assert_eq!(tags("раз два", ""), [ClipTag::Errors, ClipTag::Empty]);
        let failed = ClipResult::new("a", "раз", 1.0, 0.1, Err("нет".into()));
        assert_eq!(
            clip_tags(&failed, &word_alignment("раз", "")),
            [ClipTag::Errors, ClipTag::Failed]
        );
    }

    #[test]
    fn a_clean_clip_has_no_tags_and_digits_are_marked() {
        assert!(tags("раз", "раз").is_empty());
        assert_eq!(
            tags("в двадцать шестом году", "в 26 году"),
            [ClipTag::Errors, ClipTag::Digits]
        );
    }
}
