use nixyon_core::Word;

// Записаны кодами, чтобы в исходнике не было типографских символов.
const EM_DASH: char = '\u{2014}';
const EN_DASH: char = '\u{2013}';
const ELLIPSIS: char = '\u{2026}';
const CLOSING: &[char] = &['.', ',', '!', '?', ':', ';', ')', ']', '»', ELLIPSIS];
const OPENING: &[char] = &['(', '[', '«', '"', '„'];
/// Шаг кадра распознавателя: токен выдаётся на кадре, слово звучит до конца кадра
/// своего последнего токена.
const FRAME_SEC: f64 = 0.04;

/// Собирает слова со временем из токенов распознавателя.
///
/// Новое слово начинается после токена-разделителя (пробел или `▁`) или с токена,
/// у которого `▁`/пробел в начале. У GigaAM без пунктуации токены идут по буквам,
/// а пробел приходит отдельным токеном. Закрывающие знаки клеятся к предыдущему
/// слову, открывающие к следующему, тире становится отдельным словом.
///
/// `times`: время токенов в секундах от начала куска. Если токенов и времён
/// разное число, слова не собираются (None): текст хода тогда берётся как есть.
///
/// Конец слова: конец кадра его последнего токена с буквами, но не позже начала
/// следующего слова и конца куска. Пауза после слова и хвост куска речью не считаются;
/// знаки препинания модель выдаёт позже слова, поэтому конец они не сдвигают.
pub fn tokens_to_words(
    tokens: &[String],
    times: &[f32],
    seg_start: f64,
    seg_end: f64,
) -> Option<Vec<Word>> {
    if tokens.len() != times.len() {
        return None;
    }
    let mut words: Vec<Word> = Vec::new();
    // Время последнего токена с буквами у каждого слова.
    let mut last_times: Vec<f64> = Vec::new();
    let mut boundary = true;
    let mut opening = String::new();
    let mut opening_time: Option<f64> = None;

    for (token, &time) in tokens.iter().zip(times) {
        let t = (seg_start + f64::from(time)).clamp(seg_start, seg_end);
        let marks_start = token.starts_with(['▁', ' ']);
        let piece = token.trim_start_matches(['▁', ' ']);
        if piece.is_empty() {
            boundary = true;
            continue;
        }
        if all_in(piece, CLOSING) && !words.is_empty() {
            if let Some(last) = words.last_mut() {
                last.text.push_str(piece);
            }
            boundary = false;
            continue;
        }
        if all_in(piece, OPENING) {
            opening.push_str(piece);
            opening_time.get_or_insert(t);
            boundary = true;
            continue;
        }
        if all_in(piece, &[EM_DASH, EN_DASH]) {
            words.push(word(piece.to_string(), t));
            last_times.push(t);
            boundary = true;
            continue;
        }
        let starts_new = boundary || marks_start || !opening.is_empty() || words.is_empty();
        boundary = false;
        if starts_new {
            let start = opening_time.take().unwrap_or(t);
            words.push(word(std::mem::take(&mut opening) + piece, start));
            last_times.push(t);
        } else if let Some(last) = words.last_mut() {
            last.text.push_str(piece);
            if let Some(time) = last_times.last_mut() {
                *time = t;
            }
        }
    }
    if !opening.is_empty() {
        let start = opening_time.unwrap_or(seg_end);
        words.push(word(opening, start));
        last_times.push(start);
    }
    for i in 0..words.len() {
        let limit = words.get(i + 1).map_or(seg_end, |next| next.start);
        let end = (last_times[i] + FRAME_SEC).min(limit).min(seg_end);
        words[i].end = end.max(words[i].start);
    }
    Some(words)
}

fn word(text: String, start: f64) -> Word {
    Word {
        text,
        start,
        end: start,
    }
}

fn all_in(piece: &str, set: &[char]) -> bool {
    piece.chars().all(|c| set.contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(tokens: &[&str], seg_start: f64, seg_end: f64) -> Vec<(String, f64, f64)> {
        let tokens: Vec<String> = tokens.iter().map(|t| t.to_string()).collect();
        let times: Vec<f32> = (0..tokens.len()).map(|i| i as f32 * 0.04).collect();
        tokens_to_words(&tokens, &times, seg_start, seg_end)
            .unwrap()
            .into_iter()
            .map(|w| (w.text, w.start, w.end))
            .collect()
    }

    fn texts(tokens: &[&str]) -> Vec<String> {
        run(tokens, 0.0, 10.0).into_iter().map(|w| w.0).collect()
    }

    #[test]
    fn letter_tokens_with_separate_space_tokens_give_separate_words() {
        // Так отдаёт GigaAM v3 без пунктуации: «у лукоморья дуб».
        let tokens = [
            "у", " ", "л", "у", "к", "о", "м", "о", "р", "ь", "я", " ", "д", "у", "б",
        ];
        assert_eq!(texts(&tokens), ["у", "лукоморья", "дуб"]);
    }

    #[test]
    fn word_times_start_at_first_letter_and_end_with_its_last_letter_frame() {
        let words = run(&["д", "а", " ", "н", "е", "т"], 1.0, 2.0);
        assert_eq!(words[0].0, "да");
        assert!((words[0].1 - 1.0).abs() < 1e-9);
        assert!((words[0].2 - 1.08).abs() < 1e-6);
        assert!((words[1].1 - 1.12).abs() < 1e-6);
        assert!((words[1].2 - 1.24).abs() < 1e-6);
    }

    #[test]
    fn a_pause_after_a_word_and_the_tail_of_the_segment_are_not_speech() {
        // «да» в начале куска, пауза почти в секунду, «нет», затем тишина до конца куска.
        let tokens: Vec<String> = ["д", "а", " ", "н", "е", "т", "."]
            .iter()
            .map(|t| t.to_string())
            .collect();
        let times = [0.0, 0.04, 0.08, 1.0, 1.04, 1.08, 1.6];
        let words = tokens_to_words(&tokens, &times, 10.0, 13.0).unwrap();
        let spans: Vec<(f64, f64)> = words.iter().map(|w| (w.start, w.end)).collect();
        assert_eq!(words[1].text, "нет.");
        assert!((spans[0].1 - 10.08).abs() < 1e-6, "{spans:?}");
        assert!((spans[1].0 - 11.0).abs() < 1e-6, "{spans:?}");
        assert!((spans[1].1 - 11.12).abs() < 1e-6, "{spans:?}");
    }

    #[test]
    fn a_word_never_ends_after_the_next_one_starts() {
        let tokens: Vec<String> = ["▁а", "▁б"].iter().map(|t| t.to_string()).collect();
        let words = tokens_to_words(&tokens, &[0.0, 0.02], 0.0, 1.0).unwrap();
        assert!((words[0].end - 0.02).abs() < 1e-9);
        assert!((words[1].end - 0.06).abs() < 1e-9);
    }

    #[test]
    fn bpe_tokens_with_word_start_marker() {
        assert_eq!(texts(&["▁при", "вет", "▁ми", "р"]), ["привет", "мир"]);
    }

    #[test]
    fn closing_punctuation_sticks_to_previous_word() {
        assert_eq!(
            texts(&["▁да", ",", "▁нет", "».", "▁ну", "▁..."]),
            ["да,", "нет».", "ну..."]
        );
    }

    #[test]
    fn opening_quote_sticks_to_next_word_and_takes_its_time() {
        let words = run(&["▁он", "▁«", "сказал", "»"], 0.0, 1.0);
        assert_eq!(
            words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>(),
            ["он", "«сказал»"]
        );
        assert!((words[1].1 - 0.04).abs() < 1e-6);
    }

    #[test]
    fn dash_is_its_own_word() {
        let dash = format!("▁{EM_DASH}");
        assert_eq!(
            texts(&["▁это", &dash, "▁мы"]),
            ["это", &EM_DASH.to_string(), "мы"]
        );
    }

    #[test]
    fn hyphen_inside_word_is_kept() {
        assert_eq!(texts(&["▁кто", "-", "то"]), ["кто-то"]);
    }

    #[test]
    fn leading_punctuation_and_separators_do_not_break() {
        assert_eq!(texts(&[" ", ",", "а"]), [",а"]);
        assert!(texts(&[" ", "▁"]).is_empty());
        assert!(texts(&[]).is_empty());
    }

    #[test]
    fn mismatched_token_and_time_counts_give_no_words() {
        let tokens = vec!["а".to_string(), "б".to_string()];
        assert_eq!(tokens_to_words(&tokens, &[0.0], 0.0, 1.0), None);
    }

    #[test]
    fn times_outside_segment_are_clamped() {
        let tokens = vec!["а".to_string()];
        let words = tokens_to_words(&tokens, &[99.0], 1.0, 2.0).unwrap();
        assert_eq!((words[0].start, words[0].end), (2.0, 2.0));
    }
}
