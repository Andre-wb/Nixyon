/// Слово текста: приведённый вид и место в исходной строке в единицах UTF-16, как
/// считает строки JavaScript.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WordToken {
    pub norm: String,
    pub start: usize,
    pub end: usize,
}

/// Приведённый вид слова: нижний регистр, ё как е.
pub(crate) fn normalize(word: &str) -> String {
    word.chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ё' { 'е' } else { c })
        .collect()
}

/// Слова: подряд идущие буквы и цифры. Знаки препинания и пробелы разделяют слова.
pub(crate) fn word_tokens(text: &str) -> Vec<WordToken> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut start = 0;
    let mut offset = 0;
    for c in text.chars() {
        if c.is_alphanumeric() {
            if current.is_empty() {
                start = offset;
            }
            current.push(c);
        } else if !current.is_empty() {
            tokens.push(WordToken {
                norm: normalize(&current),
                start,
                end: offset,
            });
            current.clear();
        }
        offset += c.len_utf16();
    }
    if !current.is_empty() {
        tokens.push(WordToken {
            norm: normalize(&current),
            start,
            end: offset,
        });
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norms(text: &str) -> Vec<String> {
        word_tokens(text).into_iter().map(|t| t.norm).collect()
    }

    #[test]
    fn punctuation_splits_words_and_case_and_yo_are_normalized() {
        assert_eq!(
            norms("Ёлка, ЁЖ! кто-то «Да»"),
            vec!["елка", "еж", "кто", "то", "да"]
        );
    }

    #[test]
    fn digits_are_words() {
        assert_eq!(
            norms("в 2026 году, 8,5 м"),
            vec!["в", "2026", "году", "8", "5", "м"]
        );
    }

    #[test]
    fn offsets_are_in_utf16_units() {
        let tokens = word_tokens("😀 Анна");
        assert_eq!((tokens[0].start, tokens[0].end), (3, 7));
    }

    #[test]
    fn empty_and_punctuation_only_text_has_no_words() {
        assert!(word_tokens("").is_empty());
        assert!(word_tokens(" ,.!? ").is_empty());
    }
}
