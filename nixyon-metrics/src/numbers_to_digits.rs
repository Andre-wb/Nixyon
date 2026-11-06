use crate::{NumberWord, number_word};

/// Слова после нормализации, где числа записаны цифрами: число словами в любом падеже
/// («одна тысяча двести восемьдесят втором») становится «1282», группы цифр
/// («46 966 183») склеиваются, сокращения «тыс», «млн», «млрд» умножают число перед
/// ними. Дроби и деньги не разбираются.
pub fn numbers_to_digits(words: &[String]) -> Vec<String> {
    let tokens = join_digit_groups(words);
    let mut out = Vec::with_capacity(tokens.len());
    let mut number = Number::default();
    for (i, token) in tokens.iter().enumerate() {
        let next_multiplies = tokens
            .get(i + 1)
            .and_then(|t| number_word(t))
            .is_some_and(|w| matches!(w, NumberWord::Multiplier(_)));
        if let Some(value) = digits_value(token) {
            number.digits(value, next_multiplies, &mut out);
            continue;
        }
        match number_word(token) {
            Some(NumberWord::Cardinal(value, rank)) => number.cardinal(value, rank, &mut out),
            Some(NumberWord::Multiplier(value)) => number.multiplier(value, &mut out),
            Some(NumberWord::Ordinal(value, rank)) => number.ordinal(value, rank, &mut out),
            None => {
                number.flush(&mut out);
                out.push(token.clone());
            }
        }
    }
    number.flush(&mut out);
    out
}

// Модель пишет большие числа группами по три цифры: «46 966 183 398».
fn join_digit_groups(words: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut grouping = false;
    for word in words {
        let digits = digits_value(word).is_some();
        if grouping
            && digits
            && word.len() == 3
            && let Some(last) = out.last_mut()
        {
            last.push_str(word);
            continue;
        }
        grouping = digits && word.len() <= 3;
        out.push(word.clone());
    }
    out
}

fn digits_value(word: &str) -> Option<u64> {
    if word.is_empty() || !word.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    word.parse().ok()
}

/// Число, которое собирается из слов: сумма уже умноженных частей и текущая часть до
/// тысячи. `rank`: разряд последнего слова текущей части (0: части ещё нет).
#[derive(Debug, Default)]
struct Number {
    active: bool,
    total: u64,
    group: u64,
    rank: u8,
    /// Последний множитель; следующий множитель должен быть меньше.
    multiplier: Option<u64>,
}

impl Number {
    fn flush(&mut self, out: &mut Vec<String>) {
        if self.active {
            out.push(self.total.saturating_add(self.group).to_string());
        }
        *self = Self::default();
    }

    fn below_multiplier(&self, value: u64) -> bool {
        self.multiplier.is_none_or(|m| value < m)
    }

    fn cardinal(&mut self, value: u64, rank: u8, out: &mut Vec<String>) {
        // «двадцать пять» продолжает число, «пять двадцать» начинает новое.
        if self.active && self.rank != 0 && rank >= self.rank {
            self.flush(out);
        }
        self.active = true;
        self.group += value;
        self.rank = rank;
    }

    fn multiplier(&mut self, value: u64, out: &mut Vec<String>) {
        if self.active && !self.below_multiplier(value) {
            self.flush(out);
        }
        // «тысяча» без числа перед ней значит одну тысячу.
        let part = self.group.max(1).saturating_mul(value);
        self.active = true;
        self.total = self.total.saturating_add(part);
        self.group = 0;
        self.rank = 0;
        self.multiplier = Some(value);
    }

    fn ordinal(&mut self, value: u64, rank: u8, out: &mut Vec<String>) {
        if rank == 4 {
            // «тысячном», «двухмиллионном» и подобное: множитель, на котором число кончается.
            self.multiplier(value, out);
        } else {
            if self.active && self.rank != 0 && rank >= self.rank {
                self.flush(out);
            }
            self.active = true;
            self.group += value;
        }
        self.flush(out);
    }

    fn digits(&mut self, value: u64, next_multiplies: bool, out: &mut Vec<String>) {
        // После множителя цифры продолжают число: «39 млрд 841 млн».
        let continues =
            self.active && self.group == 0 && self.rank == 0 && self.below_multiplier(value);
        if !continues {
            self.flush(out);
        }
        if next_multiplies && value < 1_000 {
            self.active = true;
            self.group = value;
            self.rank = 3;
        } else if continues {
            self.total = self.total.saturating_add(value);
            self.flush(out);
        } else {
            out.push(value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize_words;

    fn digits(text: &str) -> String {
        numbers_to_digits(&normalize_words(text)).join(" ")
    }

    #[test]
    fn big_numbers_in_words_become_digits() {
        assert_eq!(
            digits(
                "Сорок шесть миллиардов девятьсот шестьдесят шесть миллионов сто восемьдесят три тысячи триста девяносто восемь."
            ),
            "46966183398"
        );
        assert_eq!(
            digits(
                "Тридцать четыре миллиарда восемьсот тридцать семь миллионов двадцать две тысячи тридцать два."
            ),
            "34837022032"
        );
    }

    #[test]
    fn ordinals_and_cases_end_the_number() {
        assert_eq!(
            digits(
                "Окончательный отчет об одна тысяча двести восемьдесят втором пленарном заседании"
            ),
            "окончательный отчет об 1282 пленарном заседании"
        );
        assert_eq!(digits("в двадцать три пятьдесят пять"), "в 23 55");
        assert_eq!(digits("тысяча"), "1000");
        assert_eq!(digits("пять двадцать"), "5 20");
    }

    #[test]
    fn digit_groups_and_short_multipliers_in_the_answer_are_joined() {
        assert_eq!(digits("46 966 183 398"), "46966183398");
        assert_eq!(digits("39 млрд 841 млн 676 566"), "39841676566");
        assert_eq!(
            digits("Окончательный отчёт. 1 282 пленарном"),
            "окончательный отчет 1282 пленарном"
        );
        assert_eq!(digits("в 2023 году 15 человек"), "в 2023 году 15 человек");
    }

    #[test]
    fn plain_words_stay_as_they_are() {
        assert_eq!(
            digits("он стоял не снимая пальто"),
            "он стоял не снимая пальто"
        );
        assert!(numbers_to_digits(&[]).is_empty());
    }
}
