use nixyon_cluster::min_cost_assignment;

use crate::{ErrorCount, Utterance, edit_distance, normalize_words};

/// cpWER: текст каждого спикера склеивается по порядку реплик, спикеры гипотезы
/// сопоставляются с эталоном один к одному с наименьшей суммой ошибок.
/// Лишний спикер гипотезы целиком идёт во вставки, пропущенный эталонный в удаления.
pub fn cp_word_errors(reference: &[Utterance], hypothesis: &[Utterance]) -> ErrorCount {
    let reference = words_by_speaker(reference);
    let hypothesis = words_by_speaker(hypothesis);
    let n = reference.len().max(hypothesis.len());
    let empty = Vec::new();
    let side = |groups: &[Vec<String>], i: usize| -> Vec<String> {
        groups.get(i).cloned().unwrap_or_else(|| empty.clone())
    };
    let errors: Vec<Vec<usize>> = (0..n)
        .map(|r| {
            let ref_words = side(&reference, r);
            (0..n)
                .map(|h| edit_distance(&ref_words, &side(&hypothesis, h)))
                .collect()
        })
        .collect();
    // Число ошибок далеко от 2^53, перевод в f64 точный.
    let cost: Vec<Vec<f64>> = errors
        .iter()
        .map(|row| row.iter().map(|&e| e as f64).collect())
        .collect();
    let total = min_cost_assignment(&cost)
        .into_iter()
        .enumerate()
        .map(|(r, h)| errors[r][h])
        .sum();
    ErrorCount {
        errors: total,
        reference_len: reference.iter().map(Vec::len).sum(),
    }
}

fn words_by_speaker(utterances: &[Utterance]) -> Vec<Vec<String>> {
    let mut speakers: Vec<u32> = Vec::new();
    let mut words: Vec<Vec<String>> = Vec::new();
    for u in utterances {
        let i = speakers
            .iter()
            .position(|&s| s == u.speaker)
            .unwrap_or_else(|| {
                speakers.push(u.speaker);
                words.push(Vec::new());
                speakers.len() - 1
            });
        words[i].extend(normalize_words(&u.text));
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(speaker: u32, text: &str) -> Utterance {
        Utterance {
            speaker,
            text: text.into(),
        }
    }

    #[test]
    fn right_words_and_speakers_under_other_numbers_have_zero_error() {
        let r = [u(1, "привет"), u(2, "добрый день"), u(1, "начнём")];
        let h = [u(5, "Привет!"), u(9, "Добрый день."), u(5, "Начнём.")];
        assert_eq!(
            cp_word_errors(&r, &h),
            ErrorCount {
                errors: 0,
                reference_len: 4
            }
        );
    }

    #[test]
    fn speakers_are_matched_by_text_not_by_order_of_appearance() {
        let r = [u(1, "раз два"), u(2, "три")];
        let h = [u(1, "три"), u(2, "раз два")];
        assert_eq!(cp_word_errors(&r, &h).errors, 0);
    }

    #[test]
    fn words_given_to_the_wrong_speaker_count_twice() {
        let r = [u(1, "раз два"), u(2, "три")];
        let h = [u(1, "раз два три")];
        // У спикера 1 лишнее «три», у спикера 2 оно пропало.
        assert_eq!(cp_word_errors(&r, &h).errors, 2);
    }

    #[test]
    fn extra_hypothesis_speaker_is_all_insertions() {
        let r = [u(1, "раз два")];
        let h = [u(1, "раз два"), u(2, "шум шум шум")];
        assert_eq!(cp_word_errors(&r, &h).errors, 3);
    }

    #[test]
    fn plain_recognition_errors_count_as_in_wer() {
        let r = [u(1, "я иду домой")];
        let h = [u(1, "я еду домой")];
        assert_eq!(cp_word_errors(&r, &h).errors, 1);
    }

    #[test]
    fn empty_sides() {
        assert_eq!(cp_word_errors(&[], &[]), ErrorCount::default());
        assert_eq!(cp_word_errors(&[u(1, "а б")], &[]).errors, 2);
    }
}
