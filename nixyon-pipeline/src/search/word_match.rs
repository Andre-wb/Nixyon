use rust_stemmers::{Algorithm, Stemmer};

use super::typo_distance::{allowed_typos, typo_distance};
use super::word_tokens::WordToken;

/// Слово запроса короче этого не ищется по началу слова: иначе «в» находило бы всё.
const MIN_PREFIX_LEN: usize = 3;

/// Насколько слово текста совпало со словом запроса.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchKind {
    /// Слово то же или начинается со слова запроса.
    Exact,
    /// Та же основа слова или опечатка в пределах допуска.
    Fuzzy,
}

/// Слово запроса с заранее посчитанными буквами и основой.
pub(crate) struct QueryWord {
    norm: String,
    chars: Vec<char>,
    stem: String,
    stem_chars: Vec<char>,
}

/// Сравнивает слова запроса со словами текста.
pub(crate) struct WordMatcher {
    stemmer: Stemmer,
}

impl WordMatcher {
    pub fn russian() -> Self {
        WordMatcher {
            stemmer: Stemmer::create(Algorithm::Russian),
        }
    }

    pub fn query_word(&self, norm: String) -> QueryWord {
        let stem = self.stemmer.stem(&norm).into_owned();
        QueryWord {
            chars: norm.chars().collect(),
            stem_chars: stem.chars().collect(),
            stem,
            norm,
        }
    }

    pub fn stem(&self, norm: &str) -> String {
        self.stemmer.stem(norm).into_owned()
    }

    /// `text_stem`: основа слова текста, посчитанная один раз на слово.
    pub fn compare(
        &self,
        query: &QueryWord,
        word: &WordToken,
        text_stem: &str,
    ) -> Option<MatchKind> {
        let len = query.chars.len();
        if word.norm == query.norm || (len >= MIN_PREFIX_LEN && word.norm.starts_with(&query.norm))
        {
            return Some(MatchKind::Exact);
        }
        if query.stem == text_stem {
            return Some(MatchKind::Fuzzy);
        }
        let max = allowed_typos(len);
        if max == 0 {
            return None;
        }
        let text: Vec<char> = word.norm.chars().collect();
        if typo_distance(&query.chars, &text, max).is_some() {
            return Some(MatchKind::Fuzzy);
        }
        // Опечатка в начале длинного слова: «встерч» находит «встречами».
        if text.len() > len && typo_distance(&query.chars, &text[..len], max).is_some() {
            return Some(MatchKind::Fuzzy);
        }
        // Опечатка и другое окончание сразу: «встерчу» находит «встреча».
        let stem: Vec<char> = text_stem.chars().collect();
        typo_distance(&query.stem_chars, &stem, max).map(|_| MatchKind::Fuzzy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::word_tokens::normalize;

    fn kind(query: &str, text: &str) -> Option<MatchKind> {
        let m = WordMatcher::russian();
        let q = m.query_word(normalize(query));
        let norm = normalize(text);
        let stem = m.stem(&norm);
        let word = WordToken {
            norm,
            start: 0,
            end: 0,
        };
        m.compare(&q, &word, &stem)
    }

    #[test]
    fn same_word_and_word_beginning_are_exact() {
        assert_eq!(kind("Встреча", "встреча"), Some(MatchKind::Exact));
        assert_eq!(kind("встреч", "встречами"), Some(MatchKind::Exact));
        assert_eq!(kind("елка", "Ёлка"), Some(MatchKind::Exact));
    }

    #[test]
    fn short_query_word_does_not_match_by_beginning() {
        assert_eq!(kind("вс", "встреча"), None);
        assert_eq!(kind("в", "в"), Some(MatchKind::Exact));
    }

    #[test]
    fn other_form_of_the_same_word_is_fuzzy() {
        assert_eq!(kind("встречами", "встречу"), Some(MatchKind::Fuzzy));
        assert_eq!(kind("газеты", "газета"), Some(MatchKind::Fuzzy));
    }

    #[test]
    fn typos_within_allowance_are_fuzzy() {
        assert_eq!(kind("встерча", "встреча"), Some(MatchKind::Fuzzy));
        assert_eq!(kind("Скайльс", "Скайлс"), Some(MatchKind::Fuzzy));
        assert_eq!(kind("встерч", "встречами"), Some(MatchKind::Fuzzy));
    }

    #[test]
    fn typo_together_with_other_ending_is_fuzzy() {
        assert_eq!(kind("встерчу", "встреча"), Some(MatchKind::Fuzzy));
    }

    #[test]
    fn short_words_need_exact_spelling() {
        assert_eq!(kind("дом", "дым"), None);
    }

    #[test]
    fn different_words_do_not_match() {
        assert_eq!(kind("шёл", "идти"), None);
        assert_eq!(kind("встреча", "задача"), None);
    }
}
