use crate::digit_tails::drop_digit_tails;
use crate::number_units::canonical_units;
use crate::spoken_symbols::spell_symbols;
use crate::{normalize_words, numbers_to_digits};

/// Слова текста для сравнения, где числа не зависят от записи: знаки словами, окончания
/// порядковых чисел и нулевые минуты убраны, числа цифрами, единицы после числа в одной
/// форме. «2 ₽» и «два рубля», «314-й» и «триста четырнадцатый» совпадают.
pub fn digit_words(text: &str) -> Vec<String> {
    canonical_units(numbers_to_digits(&drop_digit_tails(normalize_words(
        &spell_symbols(text),
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(reference: &str, hypothesis: &str) {
        assert_eq!(
            digit_words(reference),
            digit_words(hypothesis),
            "{reference} | {hypothesis}"
        );
    }

    #[test]
    fn number_formats_from_the_worst_clips_match() {
        same("Погребец на два рубля", "Погребец на 2 ₽");
        same("на все сто процентов", "на все 100%");
        same("на один градус", "на 1°");
        same(
            "прибыть в триста четырнадцатый кабинет",
            "прибыть в 314-й кабинет",
        );
        same("я жду завтра с одиннадцати", "я жду завтра с 11:00");
        same(
            "надпись рузаево шестнадцать километров",
            "Надпись Рузаево, 16 км.",
        );
        same("куплю его за двести долларов", "куплю его за$200");
        same("я тебе даже за две тыщи", "я тебе даже за 2 000");
        same("я просто так песят тебе дам", "Я просто так 50 тебе дам");
        same(
            "сорок шесть миллиардов девятьсот шестьдесят шесть миллионов",
            "46 966 000 000",
        );
    }

    #[test]
    fn real_recognition_errors_still_differ() {
        assert_ne!(digit_words("сто рублей"), digit_words("200 ₽"));
        assert_ne!(digit_words("семь метров"), digit_words("7 км"));
    }
}
