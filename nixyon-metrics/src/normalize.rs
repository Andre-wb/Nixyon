/// Слова для WER: нижний регистр, ё как е, всё кроме букв и цифр считается
/// разделителем. Числа не переводятся: «2026» и «две тысячи двадцать шесть» разные.
pub fn normalize_words(text: &str) -> Vec<String> {
    let cleaned: String = text
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ё' => 'е',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    cleaned.split_whitespace().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_yo_and_punctuation_do_not_matter() {
        assert_eq!(
            normalize_words("Ёлка, «Привет»… Всё: ЕЩЁ?"),
            ["елка", "привет", "все", "еще"]
        );
    }

    #[test]
    fn hyphen_and_apostrophe_split_words_on_both_sides_alike() {
        assert_eq!(normalize_words("кто-то"), ["кто", "то"]);
        assert_eq!(normalize_words("don't"), ["don", "t"]);
    }

    #[test]
    fn digits_stay_as_written() {
        assert_eq!(normalize_words("В 2026 году"), ["в", "2026", "году"]);
    }

    #[test]
    fn blank_text_has_no_words() {
        assert!(normalize_words("  ,. \n").is_empty());
    }
}
