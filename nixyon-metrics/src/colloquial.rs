const VOWELS: &[char] = &['а', 'е', 'и', 'о', 'у', 'ы', 'э', 'ю', 'я'];

/// Разговорные написания, которыми записаны эталоны звонков и YouTube, и обычное слово.
/// Обе стороны сравнения приводятся одинаково, поэтому «те» (местоимение) и «те» (тебе)
/// совпадут и так.
const COLLOQUIAL: &[(&str, &str)] = &[
    ("щас", "сейчас"),
    ("щя", "сейчас"),
    ("че", "что"),
    ("чо", "что"),
    ("шо", "что"),
    ("чето", "чтото"),
    ("ниче", "ничего"),
    ("те", "тебе"),
    ("тя", "тебя"),
    ("токо", "только"),
    ("тока", "только"),
    ("ваще", "вообще"),
    ("воще", "вообще"),
    ("наверно", "наверное"),
    ("бля", "блядь"),
    ("блять", "блядь"),
    ("алле", "алло"),
    ("але", "алло"),
    ("ж", "же"),
    ("ль", "ли"),
    ("скока", "сколько"),
    ("скоко", "сколько"),
    ("ды", "да"),
];

/// Слово в обычном написании: растянутые гласные схлопнуты («нуу», «чтоо»), разговорная
/// форма заменена обычной («щас», «че»). Согласные не схлопываются: «одиннадцать» и
/// другие слова с двойной согласной должны остаться словами чисел.
pub(crate) fn plain_form(word: &str) -> String {
    let collapsed = collapse_vowels(word);
    // Обычное слово из словаря схлопывается так же, как распознанное: «ваще» даёт
    // «вообще», а «вообще» с другой стороны уже стало «вобще».
    COLLOQUIAL
        .iter()
        .find(|(spoken, _)| *spoken == collapsed)
        .map_or(collapsed, |(_, plain)| collapse_vowels(plain))
}

fn collapse_vowels(word: &str) -> String {
    let mut collapsed = String::with_capacity(word.len());
    let mut last: Option<char> = None;
    for c in word.chars() {
        if last == Some(c) && VOWELS.contains(&c) {
            continue;
        }
        collapsed.push(c);
        last = Some(c);
    }
    collapsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretched_vowels_collapse_and_spoken_forms_become_plain() {
        assert_eq!(plain_form("нуу"), "ну");
        assert_eq!(plain_form("чтоо"), "что");
        assert_eq!(plain_form("ваам"), "вам");
        assert_eq!(plain_form("щас"), "сейчас");
        assert_eq!(plain_form("че"), "что");
        assert_eq!(plain_form("блять"), "блядь");
        assert_eq!(plain_form("наверно"), "наверное");
        assert_eq!(plain_form("скока"), "сколько");
        assert_eq!(plain_form("ж"), "же");
    }

    #[test]
    fn doubled_consonants_and_ordinary_words_stay() {
        assert_eq!(plain_form("одиннадцать"), "одиннадцать");
        assert_eq!(plain_form("класс"), "класс");
        assert_eq!(plain_form("сейчас"), "сейчас");
        assert_eq!(plain_form("что"), "что");
    }

    #[test]
    fn a_spoken_form_matches_its_plain_word_even_with_a_double_vowel() {
        assert_eq!(plain_form("ваще"), plain_form("вообще"));
        assert_eq!(plain_form("воще"), plain_form("вообще"));
    }
}
