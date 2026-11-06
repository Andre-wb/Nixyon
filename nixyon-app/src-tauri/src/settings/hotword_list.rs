/// Наибольшая длина слова или фразы в знаках, как у имени спикера.
const MAX_CHARS: usize = 64;
/// Наибольшее число слов в списке: длинный список замедляет поиск по лучам.
pub const MAX_HOTWORDS: usize = 100;

#[derive(Debug, PartialEq, Eq)]
pub enum HotwordListError {
    TooLong,
    TooMany,
}

/// Список горячих слов из текста по одному в строке: управляющие знаки становятся
/// пробелами, пробелы по краям и пустые строки убираются, повторы без учёта регистра
/// остаются один раз, в порядке ввода.
pub fn parse_hotwords(text: &str) -> Result<Vec<String>, HotwordListError> {
    let mut words: Vec<String> = Vec::new();
    for line in text.lines() {
        let clean: String = line
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let word = clean.split_whitespace().collect::<Vec<_>>().join(" ");
        if word.is_empty() {
            continue;
        }
        if word.chars().count() > MAX_CHARS {
            return Err(HotwordListError::TooLong);
        }
        if !words
            .iter()
            .any(|w| w.to_lowercase() == word.to_lowercase())
        {
            words.push(word);
        }
    }
    if words.len() > MAX_HOTWORDS {
        return Err(HotwordListError::TooMany);
    }
    Ok(words)
}

/// Слова для распознавания: имена из голосов участников, затем список из настроек,
/// без повторов (без учёта регистра).
pub fn merge_hotwords(names: &[String], list: &[String]) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in names.iter().chain(list) {
        if !words
            .iter()
            .any(|w| w.to_lowercase() == word.to_lowercase())
        {
            words.push(word.clone());
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_trimmed_deduplicated_and_kept_in_order() {
        let text = "  Арчибальд \n\nквартальный   отчёт\nарчибальд\nСкайлс\n";
        assert_eq!(
            parse_hotwords(text).unwrap(),
            ["Арчибальд", "квартальный отчёт", "Скайлс"]
        );
        assert!(parse_hotwords("").unwrap().is_empty());
        assert_eq!(parse_hotwords("ООО\u{7}Рога").unwrap(), ["ООО Рога"]);
    }

    #[test]
    fn too_long_word_or_too_many_words_are_refused() {
        assert_eq!(
            parse_hotwords(&"я".repeat(65)),
            Err(HotwordListError::TooLong)
        );
        let many: String = (0..=MAX_HOTWORDS).map(|i| format!("слово{i}\n")).collect();
        assert_eq!(parse_hotwords(&many), Err(HotwordListError::TooMany));
    }

    #[test]
    fn names_come_first_and_repeats_are_dropped() {
        let names = ["Анна".to_string(), "Борис".to_string()];
        let list = ["анна".to_string(), "Скайлс".to_string()];
        assert_eq!(merge_hotwords(&names, &list), ["Анна", "Борис", "Скайлс"]);
    }
}
