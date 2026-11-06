use crate::colloquial::plain_form;
use crate::normalize_words;
use crate::particles::split_particle;

/// Слова для мягкого WER: как `normalize_words`, но слово через дефис склеено
/// («какие-то» и «какието» совпадают, в части наборов дефисы выброшены), а частица после
/// «что», «кто», «как» и подобных снова отделена («что-то», «чтото» и «что то» дают одно
/// и то же), междометия
/// убраны («э-э», «эээ», «мм», «хм»): одни наборы их записывают, другие нет; растянутые
/// гласные и разговорные формы приведены к обычному слову («нуу», «щас»).
pub fn lenient_words(text: &str) -> Vec<String> {
    normalize_words(&join_hyphenated(text))
        .into_iter()
        .flat_map(|w| split_repeats(&w))
        .filter(|w| !is_filler(w))
        .map(|w| plain_form(&w))
        .flat_map(|w| split_particle(&w))
        .collect()
}

// Повтор короткого слова через дефис склейка превратила бы в одно слово: «да-да-да» в
// «дадада». Такое слово снова делится на повторы, как его пишут без дефиса.
fn split_repeats(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    for len in 2..=3 {
        if chars.len() >= 2 * len && chars.len().is_multiple_of(len) {
            let unit = &chars[..len];
            if chars.chunks(len).all(|c| c == unit) {
                let unit: String = unit.iter().collect();
                return vec![unit; chars.len() / len];
            }
        }
    }
    vec![word.to_owned()]
}

// Дефис между буквами убирается; тире между словами (с пробелами) и дефис рядом с
// цифрой остаются разделителем: «314-й» это число и окончание, «2-3» два числа.
fn join_hyphenated(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let hyphen = matches!(c, '-' | '\u{2010}' | '\u{2011}');
        let between = i > 0
            && chars[i - 1].is_alphabetic()
            && chars.get(i + 1).is_some_and(|n| n.is_alphabetic());
        if !(hyphen && between) {
            out.push(c);
        }
    }
    out
}

/// Междометие-заминка. Одиночные «а» и «и» союзы, поэтому они междометие только
/// повторённые («аа», «иии»); «ну», «угу» и «ага» остаются: это слова-ответы.
pub fn is_filler(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    let all = |c: char| chars.iter().all(|&x| x == c);
    let head_then_m =
        |head: char| chars.len() >= 2 && chars[0] == head && chars[1..].iter().all(|&x| x == 'м');
    !chars.is_empty()
        && (all('э')
            || all('м')
            || all('ы')
            || (chars.len() >= 2 && (all('а') || all('и') || all('у')))
            || head_then_m('э')
            || head_then_m('х'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphenated_words_are_joined_and_dashes_still_split() {
        assert_eq!(lenient_words("Какие-то вещи"), ["какие", "то", "вещи"]);
        assert_eq!(lenient_words("какието вещи"), ["какие", "то", "вещи"]);
        assert_eq!(lenient_words("раз - два"), ["раз", "два"]);
        assert_eq!(
            lenient_words("да-да-да, ну-ну"),
            ["да", "да", "да", "ну", "ну"]
        );
        assert_eq!(lenient_words("да да"), lenient_words("да-да"));
        assert_eq!(
            lenient_words("по-русски, кто-нибудь"),
            ["порусски", "кто", "нибудь"]
        );
    }

    #[test]
    fn a_particle_written_apart_matches_the_hyphen_and_the_joined_spelling() {
        assert_eq!(
            lenient_words("что то быстро"),
            lenient_words("Что-то быстро")
        );
        assert_eq!(
            lenient_words("чтото быстро"),
            lenient_words("что-то быстро")
        );
        assert_eq!(lenient_words("все таки"), lenient_words("всё-таки"));
        assert_eq!(lenient_words("чето"), lenient_words("что-то"));
    }

    #[test]
    fn fillers_are_dropped_but_short_conjunctions_and_answers_stay() {
        assert_eq!(
            lenient_words("Э-э, ну, ааа, мы иии хмм эмм пошли"),
            ["ну", "мы", "пошли"]
        );
        assert_eq!(lenient_words("а и угу ага"), ["а", "и", "угу", "ага"]);
        assert!(is_filler("ммм"));
        assert!(is_filler("ы"));
        assert!(!is_filler("мама"));
        assert!(!is_filler("эх"));
    }
}
