use nixyon_core::{Turn, Word};

/// Время слов хода: своё, если движок его дал, иначе время хода делится между
/// словами текста пропорционально числу букв в слове.
pub fn spread_words(turn: &Turn) -> Vec<Word> {
    if let Some(words) = &turn.words {
        return words.clone();
    }
    let texts: Vec<&str> = turn.text.split_whitespace().collect();
    let total: usize = texts.iter().map(|t| t.chars().count()).sum();
    if total == 0 {
        return Vec::new();
    }
    let span = (turn.end - turn.start).max(0.0);
    let mut done = 0usize;
    texts
        .into_iter()
        .map(|text| {
            let start = turn.start + span * done as f64 / total as f64;
            done += text.chars().count();
            let end = turn.start + span * done as f64 / total as f64;
            Word {
                text: text.to_string(),
                start,
                end,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(text: &str, words: Option<Vec<Word>>) -> Turn {
        Turn {
            id: 1,
            speaker: 1,
            start: 10.0,
            end: 16.0,
            text: text.into(),
            words,
            flags: Vec::new(),
            emotion: None,
        }
    }

    #[test]
    fn missing_word_times_are_spread_by_letters() {
        let words = spread_words(&turn("да  привет", None));
        assert_eq!(words.len(), 2);
        assert_eq!((words[0].start, words[0].end), (10.0, 11.5));
        assert_eq!((words[1].start, words[1].end), (11.5, 16.0));
        assert_eq!(words[1].text, "привет");
    }

    #[test]
    fn known_word_times_are_kept_and_empty_text_gives_nothing() {
        let own = vec![Word {
            text: "да".into(),
            start: 10.5,
            end: 11.0,
        }];
        assert_eq!(spread_words(&turn("да", Some(own.clone()))), own);
        assert!(spread_words(&turn("   ", None)).is_empty());
    }
}
