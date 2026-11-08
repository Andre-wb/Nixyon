mod typo_distance;
mod word_match;
mod word_tokens;

use nixyon_core::Transcript;

pub use word_match::MatchKind;

use word_match::{QueryWord, WordMatcher};
use word_tokens::{WordToken, word_tokens};

/// Найденное место: ход и отрезок его текста в единицах UTF-16 (как индексы строк
/// JavaScript), от начала первого слова фразы до конца последнего.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub turn_id: u32,
    pub start: usize,
    pub end: usize,
    pub kind: MatchKind,
}

/// Ищет фразу запроса в текстах реплик: слова запроса подряд и по порядку. Регистр и
/// разница ё и е не важны, знаки препинания пропускаются. Слово совпадает точно (то же
/// слово или его начало) или нечётко (та же основа, опечатка). Сначала идут точные
/// совпадения, затем нечёткие; внутри каждой группы по порядку ходов. Основу слова и
/// запроса задаёт язык встречи.
pub fn search(transcript: &Transcript, query: &str) -> Vec<SearchHit> {
    let matcher = WordMatcher::of(transcript.language);
    let query: Vec<QueryWord> = word_tokens(query)
        .into_iter()
        .map(|w| matcher.query_word(w.norm))
        .collect();
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for turn in &transcript.turns {
        let words = word_tokens(&turn.text);
        let stems: Vec<String> = words.iter().map(|w| matcher.stem(&w.norm)).collect();
        hits.extend(
            phrase_hits(&matcher, &query, &words, &stems)
                .into_iter()
                .map(|(start, end, kind)| SearchHit {
                    turn_id: turn.id,
                    start,
                    end,
                    kind,
                }),
        );
    }
    // Сортировка устойчивая: внутри группы остаётся порядок ходов.
    hits.sort_by_key(|h| h.kind);
    hits
}

/// Совпадения фразы внутри одного хода; совпадения не накладываются друг на друга.
fn phrase_hits(
    matcher: &WordMatcher,
    query: &[QueryWord],
    words: &[WordToken],
    stems: &[String],
) -> Vec<(usize, usize, MatchKind)> {
    let mut hits = Vec::new();
    let mut i = 0;
    while i + query.len() <= words.len() {
        let kinds: Option<Vec<MatchKind>> = query
            .iter()
            .enumerate()
            .map(|(k, q)| matcher.compare(q, &words[i + k], &stems[i + k]))
            .collect();
        match kinds.and_then(|k| k.into_iter().max()) {
            Some(kind) => {
                let last = &words[i + query.len() - 1];
                hits.push((words[i].start, last.end, kind));
                i += query.len();
            }
            None => i += 1,
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;
    use nixyon_core::SpeechLanguage;

    fn found(turns: &[(u32, f64, f64, &str)], query: &str) -> Vec<(u32, usize, usize, MatchKind)> {
        search(&transcript(turns), query)
            .into_iter()
            .map(|h| (h.turn_id, h.start, h.end, h.kind))
            .collect()
    }

    #[test]
    fn english_meeting_finds_other_forms_of_an_english_word() {
        let mut t = transcript(&[
            (1, 0.0, 1.0, "We run the tests every day."),
            (2, 1.0, 2.0, "Everyone agreed on the plan."),
        ]);
        t.language = SpeechLanguage::En;
        let kinds = |t: &Transcript, query: &str| -> Vec<(u32, MatchKind)> {
            search(t, query)
                .into_iter()
                .map(|h| (h.turn_id, h.kind))
                .collect()
        };
        assert_eq!(kinds(&t, "running"), [(1, MatchKind::Fuzzy)]);
        assert_eq!(kinds(&t, "agreeing"), [(2, MatchKind::Fuzzy)]);
        assert_eq!(kinds(&t, "plans"), [(2, MatchKind::Fuzzy)]);
        // Русская встреча считает основы по-русски, английские формы там не сходятся.
        t.language = SpeechLanguage::Ru;
        assert!(kinds(&t, "running").is_empty());
    }

    #[test]
    fn empty_or_punctuation_query_finds_nothing() {
        let turns = [(1, 0.0, 1.0, "Привет, мир")];
        assert!(found(&turns, "").is_empty());
        assert!(found(&turns, " ,!? ").is_empty());
    }

    #[test]
    fn hit_points_at_the_word_in_the_original_text() {
        let hits = found(&[(1, 0.0, 1.0, "Ну, Ёлка зелёная.")], "елка");
        assert_eq!(hits, vec![(1, 4, 8, MatchKind::Exact)]);
    }

    #[test]
    fn phrase_words_must_follow_each_other() {
        let turns = [
            (1, 0.0, 1.0, "новая встреча завтра"),
            (2, 1.0, 2.0, "встреча новая"),
        ];
        assert_eq!(
            found(&turns, "новая встреча"),
            vec![(1, 0, 13, MatchKind::Exact)]
        );
    }

    #[test]
    fn exact_hits_come_before_fuzzy_ones() {
        let turns = [(1, 0.0, 1.0, "встерча"), (2, 1.0, 2.0, "встреча")];
        assert_eq!(
            found(&turns, "встреча"),
            vec![(2, 0, 7, MatchKind::Exact), (1, 0, 7, MatchKind::Fuzzy)]
        );
    }

    #[test]
    fn phrase_is_fuzzy_if_any_word_is_fuzzy() {
        let turns = [(1, 0.0, 1.0, "новые встречу")];
        assert_eq!(
            found(&turns, "новые встреча"),
            vec![(1, 0, 13, MatchKind::Fuzzy)]
        );
    }

    #[test]
    fn every_occurrence_in_a_turn_is_found() {
        let turns = [(1, 0.0, 1.0, "да, да и да")];
        assert_eq!(found(&turns, "да").len(), 3);
    }
}
