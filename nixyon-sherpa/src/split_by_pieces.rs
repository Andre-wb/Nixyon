// Записаны кодами, чтобы в исходнике не было типографских символов.
const WORD_MARK: char = '\u{2581}';
const CLOSING: &[char] = &['.', ',', '!', '?', ':', ';', ')', ']', '»', '\u{2026}'];
const OPENING: &[char] = &['(', '[', '«', '"', '„'];
const DASHES: &[char] = &['\u{2014}', '\u{2013}'];

/// Текст одного куска речи из распознанной группы; `times`: время токенов в секундах
/// от начала куска.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PieceText {
    pub text: String,
    pub tokens: Vec<String>,
    pub times: Vec<f32>,
}

/// Раздаёт токены распознанной группы по её кускам. `times`: время токенов в секундах от
/// начала группы; `pieces`: куски группы (начало, конец) в секундах от начала группы.
/// Слово уходит в кусок целиком, по времени своего первого токена; граница между
/// соседними кусками посередине паузы между ними. Закрывающий знак остаётся при слове
/// перед ним, открывающий идёт со словом после него. Правила те же, что у приложения на
/// Android (AsrGroups.kt). None, если у токенов нет времени: резать нечем.
pub(crate) fn split_by_pieces(
    tokens: &[String],
    times: &[f32],
    pieces: &[(f64, f64)],
) -> Option<Vec<PieceText>> {
    if pieces.is_empty() || tokens.len() != times.len() {
        return None;
    }
    let mut owner = vec![0usize; tokens.len()];
    let mut piece = 0usize;
    let mut boundary = true;
    let mut opening = false;
    let mut any_word = false;
    for (i, token) in tokens.iter().enumerate() {
        let body = token.trim_start_matches([WORD_MARK, ' ']);
        let all_in = |set: &[char]| body.chars().all(|c| set.contains(&c));
        // Начинается ли с этого токена новое слово (или открывающий знак перед словом).
        let starts_word = if body.is_empty() {
            boundary = true;
            false
        } else if any_word && all_in(CLOSING) {
            boundary = false;
            false
        } else if all_in(OPENING) {
            let first = !opening;
            opening = true;
            boundary = true;
            first
        } else if all_in(DASHES) {
            let first = !opening;
            any_word = true;
            boundary = true;
            first
        } else {
            let starts = boundary || body.len() != token.len() || !any_word;
            // После открывающего знака слово уже начато на нём.
            let first = starts && !opening;
            boundary = false;
            opening = false;
            any_word = true;
            first
        };
        if starts_word {
            let t = f64::from(times[i]);
            while piece + 1 < pieces.len() && t >= (pieces[piece].1 + pieces[piece + 1].0) / 2.0 {
                piece += 1;
            }
        }
        owner[i] = piece;
    }
    Some(
        pieces
            .iter()
            .enumerate()
            .map(|(k, &(start, end))| {
                let picked: Vec<usize> = (0..tokens.len()).filter(|&i| owner[i] == k).collect();
                let text: String = picked.iter().map(|&i| tokens[i].as_str()).collect();
                PieceText {
                    text: text.replace(WORD_MARK, " ").trim().to_string(),
                    tokens: picked.iter().map(|&i| tokens[i].clone()).collect(),
                    // Время в пределах куска: f32 хватает, кусок короче 20 с.
                    times: picked
                        .iter()
                        .map(|&i| (f64::from(times[i]) - start).clamp(0.0, end - start) as f32)
                        .collect(),
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(tokens: &[&str], times: &[f32], pieces: &[(f64, f64)]) -> Vec<PieceText> {
        let tokens: Vec<String> = tokens.iter().map(|t| t.to_string()).collect();
        split_by_pieces(&tokens, times, pieces).unwrap()
    }

    fn texts(parts: &[PieceText]) -> Vec<&str> {
        parts.iter().map(|p| p.text.as_str()).collect()
    }

    fn close(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
    }

    #[test]
    fn words_go_to_the_piece_where_they_start_with_times_from_that_piece() {
        let parts = split(
            &["▁При", "вет", ",", "▁как", "▁дела", "?"],
            &[0.0, 0.1, 0.3, 1.5, 1.8, 2.1],
            &[(0.0, 0.6), (1.2, 2.4)],
        );
        assert_eq!(texts(&parts), ["Привет,", "как дела?"]);
        assert_eq!(parts[0].tokens, ["▁При", "вет", ","]);
        assert!(close(&parts[0].times, &[0.0, 0.1, 0.3]));
        assert!(
            close(&parts[1].times, &[0.3, 0.6, 0.9]),
            "{:?}",
            parts[1].times
        );
    }

    #[test]
    fn a_closing_mark_stays_with_the_word_before_it_even_past_the_cut() {
        let parts = split(
            &["▁да", ".", "▁Нет"],
            &[0.1, 1.0, 1.5],
            &[(0.0, 0.5), (0.9, 2.0)],
        );
        assert_eq!(texts(&parts), ["да.", "Нет"]);
        // Знак позже конца своего куска: время прижато к концу куска.
        assert!(close(&parts[0].times, &[0.1, 0.5]));
    }

    #[test]
    fn a_word_is_never_cut_between_its_tokens() {
        let parts = split(
            &["▁пре", "крас", "но", "▁всё"],
            &[0.4, 0.8, 1.0, 1.3],
            &[(0.0, 0.6), (0.8, 1.6)],
        );
        assert_eq!(texts(&parts), ["прекрасно", "всё"]);
    }

    #[test]
    fn letter_tokens_with_space_tokens_split_on_the_space() {
        // Так отдаёт GigaAM v3 без пунктуации: буквы и пробел отдельным токеном.
        let parts = split(
            &["д", "а", " ", "н", "е", "т"],
            &[0.0, 0.04, 0.3, 1.0, 1.04, 1.08],
            &[(0.0, 0.5), (0.9, 1.5)],
        );
        assert_eq!(texts(&parts), ["да", "нет"]);
        assert_eq!(parts[1].tokens, ["н", "е", "т"]);
    }

    #[test]
    fn an_opening_mark_and_the_word_after_it_stay_together() {
        let tokens = ["▁Он", "▁сказал", ":", "▁«", "Ура", "»"];
        let pieces = [(0.0, 0.55), (0.85, 1.4)];
        let early = split(&tokens, &[0.0, 0.2, 0.5, 0.6, 0.8, 1.1], &pieces);
        assert_eq!(texts(&early), ["Он сказал: «Ура»", ""]);
        let late = split(&tokens, &[0.0, 0.2, 0.5, 0.75, 0.8, 1.1], &pieces);
        assert_eq!(texts(&late), ["Он сказал:", "«Ура»"]);
    }

    #[test]
    fn a_piece_without_words_gets_empty_text() {
        let parts = split(&["▁да", "▁нет"], &[0.1, 0.3], &[(0.0, 0.5), (1.0, 1.5)]);
        assert_eq!(texts(&parts), ["да нет", ""]);
        assert!(parts[1].tokens.is_empty());
    }

    #[test]
    fn without_token_times_there_is_nothing_to_split_by() {
        let tokens = vec!["▁да".to_string()];
        assert_eq!(
            split_by_pieces(&tokens, &[], &[(0.0, 1.0), (2.0, 3.0)]),
            None
        );
    }
}
