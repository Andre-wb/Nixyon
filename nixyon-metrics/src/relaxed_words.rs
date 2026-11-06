use crate::digit_tails::drop_digit_tails;
use crate::lenient_words;
use crate::number_units::canonical_units;
use crate::numbers_to_digits;
use crate::spoken_symbols::spell_symbols;

/// Слова для сравнения, которое не считает ошибкой запись: числа и единицы как в
/// `digit_words`, дефисы, междометия, растянутые гласные и разговорные формы как в
/// `lenient_words`. По нему отбираются худшие клипы и частые замены.
pub fn relaxed_words(text: &str) -> Vec<String> {
    canonical_units(numbers_to_digits(&drop_digit_tails(lenient_words(
        &spell_symbols(text),
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(reference: &str, hypothesis: &str) {
        assert_eq!(
            relaxed_words(reference),
            relaxed_words(hypothesis),
            "{reference} | {hypothesis}"
        );
    }

    #[test]
    fn spelling_and_number_formats_from_the_calls_match() {
        same("щас ты звонишь блять", "Сейчас ты звонишь, блядь");
        same("че те надо", "Что тебе надо?");
        same("какойто нуу чтоо", "какой-то, ну, что");
        same(
            "прибыть в триста четырнадцатый кабинет",
            "прибыть в 314-й кабинет",
        );
        same("сто рублей ааа", "100 ₽");
        same("два три часа", "2-3 часа");
    }

    #[test]
    fn real_errors_still_differ() {
        assert_ne!(relaxed_words("но мы пошли"), relaxed_words("ну мы пошли"));
        assert_ne!(relaxed_words("сто рублей"), relaxed_words("200 ₽"));
    }
}
