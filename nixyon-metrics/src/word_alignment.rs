use crate::{WordEdit, normalize_words, relaxed_words};

/// Выравнивание слов эталона и расшифровки после нормализации (`normalize_words`):
/// какие слова совпали, заменены, пропущены или лишние. Ошибок в нём столько же, сколько
/// насчитывает `word_errors`. При равной цене замена предпочитается паре «пропуск и
/// лишнее слово».
pub fn word_alignment(reference: &str, hypothesis: &str) -> Vec<WordEdit> {
    align(&normalize_words(reference), &normalize_words(hypothesis))
}

/// То же выравнивание без ошибок записи (`relaxed_words`): «десять» и «10», «щас» и
/// «сейчас» не замены. Ошибок в нём столько же, сколько у `word_errors_relaxed`.
pub fn word_alignment_relaxed(reference: &str, hypothesis: &str) -> Vec<WordEdit> {
    align(&relaxed_words(reference), &relaxed_words(hypothesis))
}

// Полная таблица цен нужна, чтобы пройти обратно и восстановить сами правки.
fn align(reference: &[String], hypothesis: &[String]) -> Vec<WordEdit> {
    let width = hypothesis.len() + 1;
    let mut cost = vec![0usize; (reference.len() + 1) * width];
    for (j, cell) in cost.iter_mut().take(width).enumerate() {
        *cell = j;
    }
    for i in 1..=reference.len() {
        cost[i * width] = i;
        for j in 1..=hypothesis.len() {
            let diagonal =
                cost[(i - 1) * width + j - 1] + usize::from(reference[i - 1] != hypothesis[j - 1]);
            cost[i * width + j] = diagonal
                .min(cost[(i - 1) * width + j] + 1)
                .min(cost[i * width + j - 1] + 1);
        }
    }
    let (mut i, mut j) = (reference.len(), hypothesis.len());
    let mut edits = Vec::with_capacity(i.max(j));
    while i > 0 || j > 0 {
        let here = cost[i * width + j];
        if i > 0
            && j > 0
            && here
                == cost[(i - 1) * width + j - 1]
                    + usize::from(reference[i - 1] != hypothesis[j - 1])
        {
            edits.push(if reference[i - 1] == hypothesis[j - 1] {
                WordEdit::Same(reference[i - 1].clone())
            } else {
                WordEdit::Replaced {
                    reference: reference[i - 1].clone(),
                    hypothesis: hypothesis[j - 1].clone(),
                }
            });
            i -= 1;
            j -= 1;
        } else if i > 0 && here == cost[(i - 1) * width + j] + 1 {
            edits.push(WordEdit::Missed(reference[i - 1].clone()));
            i -= 1;
        } else {
            edits.push(WordEdit::Extra(hypothesis[j - 1].clone()));
            j -= 1;
        }
    }
    edits.reverse();
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word_errors;

    fn same(w: &str) -> WordEdit {
        WordEdit::Same(w.into())
    }

    #[test]
    fn replaced_missed_and_extra_words_are_told_apart() {
        assert_eq!(
            word_alignment("Я иду домой сейчас.", "я еду домой"),
            [
                same("я"),
                WordEdit::Replaced {
                    reference: "иду".into(),
                    hypothesis: "еду".into()
                },
                same("домой"),
                WordEdit::Missed("сейчас".into()),
            ]
        );
        assert_eq!(
            word_alignment("да", "ну да"),
            [WordEdit::Extra("ну".into()), same("да")]
        );
    }

    #[test]
    fn empty_sides_give_only_missed_or_only_extra_words() {
        assert_eq!(
            word_alignment("раз два", ""),
            [
                WordEdit::Missed("раз".into()),
                WordEdit::Missed("два".into())
            ]
        );
        assert_eq!(word_alignment("", "раз"), [WordEdit::Extra("раз".into())]);
        assert!(word_alignment("", "").is_empty());
    }

    #[test]
    fn errors_in_the_alignment_match_word_errors() {
        let pairs = [
            (
                "мало помалу буря утихла комендантша успокоилась",
                "мало помалу буря утих",
            ),
            (
                "вечно нелепые у тебя шутки а мне папа велел",
                "вечно нелепый у тебя шум да",
            ),
            ("он стоял не снимая пальто", "стоял не снимая пальто тамана"),
            ("сорок шесть миллиардов", "46 966"),
        ];
        for (reference, hypothesis) in pairs {
            let edits = word_alignment(reference, hypothesis);
            let errors = edits.iter().filter(|e| e.is_error()).count();
            assert_eq!(
                errors,
                word_errors(reference, hypothesis).errors,
                "{reference}"
            );
            let kept = edits
                .iter()
                .filter(|e| !matches!(e, WordEdit::Extra(_)))
                .count();
            assert_eq!(kept, word_errors(reference, hypothesis).reference_len);
        }
    }
}
