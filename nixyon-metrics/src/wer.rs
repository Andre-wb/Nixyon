use crate::{
    ErrorCount, digit_words, edit_distance, lenient_words, normalize_words, relaxed_words,
};

/// Ошибки по словам после нормализации (`normalize_words`).
pub fn word_errors(reference: &str, hypothesis: &str) -> ErrorCount {
    let reference = normalize_words(reference);
    let hypothesis = normalize_words(hypothesis);
    ErrorCount {
        errors: edit_distance(&reference, &hypothesis),
        reference_len: reference.len(),
    }
}

/// Ошибки по словам, когда числа с обеих сторон записаны одинаково (`digit_words`):
/// «46 966» и «сорок шесть тысяч девятьсот шестьдесят шесть», «2 ₽» и «два рубля»
/// совпадают. Длина эталона считается после перевода, число словами в нём одно слово.
pub fn word_errors_with_digits(reference: &str, hypothesis: &str) -> ErrorCount {
    let reference = digit_words(reference);
    let hypothesis = digit_words(hypothesis);
    ErrorCount {
        errors: edit_distance(&reference, &hypothesis),
        reference_len: reference.len(),
    }
}

/// Мягкие ошибки по словам (`lenient_words`): слова через дефис склеены, междометия
/// не считаются ни в эталоне, ни в расшифровке.
pub fn word_errors_lenient(reference: &str, hypothesis: &str) -> ErrorCount {
    let reference = lenient_words(reference);
    let hypothesis = lenient_words(hypothesis);
    ErrorCount {
        errors: edit_distance(&reference, &hypothesis),
        reference_len: reference.len(),
    }
}

/// Ошибки без ошибок записи (`relaxed_words`): числа, единицы, дефисы, междометия и
/// разговорные формы не считаются. По ним отбираются худшие клипы.
pub fn word_errors_relaxed(reference: &str, hypothesis: &str) -> ErrorCount {
    let reference = relaxed_words(reference);
    let hypothesis = relaxed_words(hypothesis);
    ErrorCount {
        errors: edit_distance(&reference, &hypothesis),
        reference_len: reference.len(),
    }
}

/// Ошибки по знакам: нормализованные слова через один пробел, пробел тоже знак.
pub fn char_errors(reference: &str, hypothesis: &str) -> ErrorCount {
    let reference = joined_chars(reference);
    let hypothesis = joined_chars(hypothesis);
    ErrorCount {
        errors: edit_distance(&reference, &hypothesis),
        reference_len: reference.len(),
    }
}

fn joined_chars(text: &str) -> Vec<char> {
    normalize_words(text).join(" ").chars().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_wrong_word_of_four_is_quarter_wer() {
        let e = word_errors("Я иду домой сейчас.", "я еду домой сейчас");
        assert_eq!(
            e,
            ErrorCount {
                errors: 1,
                reference_len: 4
            }
        );
        assert_eq!(e.rate(), Some(0.25));
    }

    #[test]
    fn extra_words_can_push_wer_above_one() {
        let e = word_errors("да", "да нет нет");
        assert_eq!(e.rate(), Some(2.0));
    }

    #[test]
    fn empty_hypothesis_deletes_every_reference_word() {
        assert_eq!(word_errors("раз два", "").errors, 2);
    }

    #[test]
    fn numbers_in_words_and_in_digits_match_after_conversion() {
        let reference = "Сорок шесть миллиардов девятьсот шестьдесят шесть миллионов сто восемьдесят три тысячи триста девяносто восемь.";
        assert_eq!(word_errors(reference, "46 966 183 398").errors, 14);
        assert_eq!(
            word_errors_with_digits(reference, "46 966 183 398"),
            ErrorCount {
                errors: 0,
                reference_len: 1
            }
        );
        assert_eq!(word_errors_with_digits("я иду", "я еду").errors, 1);
    }

    #[test]
    fn hyphens_and_fillers_do_not_count_in_the_lenient_score() {
        let reference = "ааа ну какието вещи эээ";
        let hypothesis = "Э-э, ну, какие-то вещи.";
        assert_eq!(word_errors(reference, hypothesis).errors, 5);
        assert_eq!(
            word_errors_lenient(reference, hypothesis),
            ErrorCount {
                errors: 0,
                reference_len: 4
            }
        );
    }

    #[test]
    fn cer_counts_letters_after_normalization() {
        let e = char_errors("Ёж!", "еш");
        assert_eq!(
            e,
            ErrorCount {
                errors: 1,
                reference_len: 2
            }
        );
    }

    #[test]
    fn cer_counts_the_space_between_words() {
        assert_eq!(char_errors("а б", "аб").errors, 1);
        assert_eq!(char_errors("а б", "аб").reference_len, 3);
    }
}
