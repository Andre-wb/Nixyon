/// Единица или валюта в одной форме и все формы, которыми её пишут модель и эталоны.
const UNITS: &[(&str, &[&str])] = &[
    (
        "руб",
        &[
            "руб",
            "рубль",
            "рубля",
            "рублей",
            "рублю",
            "рублям",
            "рублями",
            "рублях",
        ],
    ),
    (
        "долл",
        &[
            "долл",
            "доллар",
            "доллара",
            "долларов",
            "доллару",
            "долларам",
            "долларами",
        ],
    ),
    ("евро", &["евро"]),
    (
        "процент",
        &[
            "процент",
            "процента",
            "процентов",
            "процентам",
            "процентами",
            "процентах",
        ],
    ),
    (
        "градус",
        &["градус", "градуса", "градусов", "градусам", "градусами"],
    ),
    (
        "км",
        &[
            "км",
            "километр",
            "километра",
            "километров",
            "километрам",
            "километрами",
            "километрах",
        ],
    ),
    (
        "м",
        &[
            "м",
            "метр",
            "метра",
            "метров",
            "метрам",
            "метрами",
            "метрах",
        ],
    ),
    ("см", &["см", "сантиметр", "сантиметра", "сантиметров"]),
    ("мм", &["мм", "миллиметр", "миллиметра", "миллиметров"]),
    ("кг", &["кг", "килограмм", "килограмма", "килограммов"]),
    ("г", &["г", "грамм", "грамма", "граммов"]),
    ("мг", &["мг", "миллиграмм", "миллиграмма", "миллиграммов"]),
    ("л", &["л", "литр", "литра", "литров"]),
    ("ч", &["ч", "час", "часа", "часов"]),
];

// Валюта, которую модель ставит перед числом: «$200» читается как «200 долларов».
const CURRENCY_BEFORE: &[&str] = &["долл", "евро", "руб"];

fn is_number(word: &str) -> bool {
    !word.is_empty() && word.bytes().all(|b| b.is_ascii_digit())
}

fn unit_of(word: &str) -> Option<&'static str> {
    UNITS
        .iter()
        .find(|(_, forms)| forms.contains(&word))
        .map(|&(unit, _)| unit)
}

/// Единицы и валюта после числа в одной форме: «16 км» и «шестнадцать километров»
/// совпадают. Вне числа слово не трогается: «м» бывает и междометием. Валюта перед
/// числом переносится за него.
pub(crate) fn canonical_units(words: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let word = &words[i];
        let unit = unit_of(word);
        if let Some(unit) = unit
            && CURRENCY_BEFORE.contains(&unit)
            && words.get(i + 1).is_some_and(|next| is_number(next))
            && !out.last().is_some_and(|last| is_number(last))
        {
            out.push(words[i + 1].clone());
            out.push(unit.to_owned());
            i += 2;
            continue;
        }
        match unit {
            Some(unit) if out.last().is_some_and(|last| is_number(last)) => {
                out.push(unit.to_owned())
            }
            _ => out.push(word.clone()),
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(words: &[&str]) -> Vec<String> {
        canonical_units(words.iter().map(|w| w.to_string()).collect())
    }

    #[test]
    fn units_after_a_number_take_one_form_in_any_case() {
        assert_eq!(units(&["16", "километров"]), ["16", "км"]);
        assert_eq!(units(&["7", "метров"]), ["7", "м"]);
        assert_eq!(units(&["2", "рубля"]), ["2", "руб"]);
        assert_eq!(units(&["100", "процентов"]), ["100", "процент"]);
    }

    #[test]
    fn currency_before_a_number_moves_after_it() {
        assert_eq!(units(&["за", "долл", "200"]), ["за", "200", "долл"]);
        assert_eq!(units(&["200", "долл"]), ["200", "долл"]);
    }

    #[test]
    fn unit_words_without_a_number_stay() {
        assert_eq!(units(&["м", "да"]), ["м", "да"]);
        assert_eq!(units(&["много", "часов"]), ["много", "часов"]);
    }
}
