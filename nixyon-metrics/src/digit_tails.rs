/// Окончания порядковых чисел после цифр: «314-й», «790-ю», «66-я» нормализация делит
/// на число и окончание. «м» не входит: после числа это чаще метры.
const ORDINAL_ENDINGS: &[&str] = &[
    "й", "я", "е", "ю", "х", "го", "му", "ми", "ый", "ий", "ой", "ая", "ое", "ые", "ых", "ым", "ом",
];

/// Убирает то, что модель дописывает к числу цифрами, а эталон словами не пишет:
/// окончание порядкового числа («314-й» как «314») и нулевые минуты во времени
/// («11:00» как «11»).
pub(crate) fn drop_digit_tails(words: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for word in words {
        let after_digits = out
            .last()
            .is_some_and(|last| !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()));
        if after_digits && (ORDINAL_ENDINGS.contains(&word.as_str()) || word == "00") {
            continue;
        }
        out.push(word);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize_words;

    fn tails(text: &str) -> Vec<String> {
        drop_digit_tails(normalize_words(text))
    }

    #[test]
    fn ordinal_endings_and_zero_minutes_after_digits_go() {
        assert_eq!(tails("в 314-й кабинет"), ["в", "314", "кабинет"]);
        assert_eq!(tails("790-ю телеграмму"), ["790", "телеграмму"]);
        assert_eq!(tails("с 11:00"), ["с", "11"]);
        assert_eq!(tails("в 23:55"), ["в", "23", "55"]);
    }

    #[test]
    fn the_same_words_without_a_number_before_them_stay() {
        assert_eq!(tails("я и ты"), ["я", "и", "ты"]);
        assert_eq!(tails("на 8,5 м"), ["на", "8", "5", "м"]);
    }
}
