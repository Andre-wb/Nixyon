use nixyon_core::{OVERLAP_FLAG, Turn, Word};

use crate::speaker_names::is_dash;

/// Хвост реплики переносится следующему спикеру, только если пауза между ними короче.
const MAX_GAP_SEC: f64 = 1.0;
/// Переносится не больше стольких слов: это поправка границы, а не пересказ реплики.
const MAX_TAIL_WORDS: usize = 2;

fn is_end(c: char) -> bool {
    matches!(c, '.' | '?' | '!' | '…')
}

/// Чинит границы реплик после склейки ходов:
/// 1. Фраза, разрезанная границей говорящих. Реплика кончается коротким хвостом без знака
///    конца фразы («— Вы учитесь? — Я»), а следующая реплика другого спикера начинается
///    со строчной буквы («учусь…»): хвост переходит в начало следующей реплики. Граница
///    попала внутрь слова-двух из-за того, что говорящий слова берётся по его середине.
/// 2. Реплика из одних знаков препинания («.») не остаётся отдельной.
///
/// Реплики с пометками (например, наложение голосов) не трогаются. Ходы идут по времени.
/// После правки соседние ходы одного спикера надо склеить заново (`glue_turns`).
pub(crate) fn fix_split_sentences(mut turns: Vec<Turn>) -> Vec<Turn> {
    for i in 1..turns.len() {
        let (before, after) = turns.split_at_mut(i);
        move_tail(&mut before[i - 1], &mut after[0]);
    }
    // Ход без текста остаётся, если это неразборчивая речь или перекрытие: он показывает,
    // что человек говорил.
    turns.retain(|t| !t.text.trim().is_empty() || t.is_unclear() || is_overlap(t));
    absorb_stray_punctuation(turns)
}

fn is_overlap(t: &Turn) -> bool {
    t.flags.iter().any(|f| f == OVERLAP_FLAG)
}

fn consistent_words(t: &Turn) -> bool {
    t.words
        .as_ref()
        .is_some_and(|w| w.len() == t.text.split_whitespace().count())
}

fn move_tail(a: &mut Turn, b: &mut Turn) {
    if a.speaker == b.speaker
        || !a.flags.is_empty()
        || !b.flags.is_empty()
        || b.start - a.end >= MAX_GAP_SEC
        || b.text.trim_start().starts_with(['—', '–', '-'])
        || !b
            .text
            .chars()
            .find(|c| c.is_alphanumeric())
            .is_some_and(char::is_lowercase)
    {
        return;
    }
    let text = a.text.trim_end();
    // Хвост: всё после последнего знака конца фразы.
    let cut = text
        .char_indices()
        .rev()
        .find(|&(_, c)| is_end(c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    if cut == 0 {
        return;
    }
    let tokens: Vec<&str> = text[cut..].split_whitespace().collect();
    let is_word: Vec<bool> = tokens.iter().map(|t| !is_dash(t)).collect();
    let moved: Vec<&str> = tokens
        .iter()
        .zip(&is_word)
        .filter(|(_, w)| **w)
        .map(|(t, _)| *t)
        .collect();
    if moved.is_empty() || moved.len() > MAX_TAIL_WORDS {
        return;
    }
    let moved_text = moved.join(" ");
    let keep_text = text[..cut].trim_end().to_string();
    let tail_tokens = tokens.len();

    let (new_a_words, new_b_words) = if consistent_words(a) && consistent_words(b) {
        let mut a_words = a.words.take().unwrap_or_default();
        let at = a_words.len() - tail_tokens;
        let mut b_words: Vec<Word> = a_words
            .split_off(at)
            .into_iter()
            .zip(&is_word)
            .filter(|(_, w)| **w)
            .map(|(word, _)| word)
            .collect();
        b_words.extend(b.words.take().unwrap_or_default());
        (Some(a_words), Some(b_words))
    } else {
        // Время слов не сходится с текстом: после правки его лучше не хранить.
        (None, None)
    };
    if let (Some(aw), Some(bw)) = (&new_a_words, &new_b_words) {
        if let Some(last) = aw.last() {
            a.end = a.end.min(last.end);
        }
        if let Some(first) = bw.first() {
            b.start = b.start.min(first.start);
        }
    }
    a.text = keep_text;
    b.text = format!("{moved_text} {}", b.text);
    a.words = new_a_words;
    b.words = new_b_words;
}

// Реплика без букв и цифр («.») присоединяется к предыдущей: если у той нет знака конца
// фразы, знак дописывается, иначе пропадает.
fn absorb_stray_punctuation(turns: Vec<Turn>) -> Vec<Turn> {
    let mut out: Vec<Turn> = Vec::with_capacity(turns.len());
    for turn in turns {
        let stray = turn.flags.is_empty() && !turn.text.chars().any(char::is_alphanumeric);
        match out.last_mut() {
            Some(prev) if stray => {
                let marks = turn.text.trim();
                if !prev.text.ends_with(is_end) && !marks.is_empty() && marks.chars().all(is_end) {
                    prev.text.push_str(marks);
                }
            }
            _ => out.push(turn),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{transcript, word};

    fn shape(turns: &[Turn]) -> Vec<(u32, &str)> {
        turns.iter().map(|t| (t.speaker, t.text.as_str())).collect()
    }

    #[test]
    fn the_screenshot_case_moves_the_word_and_drops_the_stray_dot() {
        let t = transcript(&[
            (1, 13.0, 16.0, "— Вы работаете или учитесь? — Я"),
            (2, 16.1, 18.0, "учусь в Московском университете."),
            (1, 18.0, 18.1, "."),
        ]);
        let fixed = fix_split_sentences(t.turns);
        assert_eq!(
            shape(&fixed),
            [
                (1, "— Вы работаете или учитесь?"),
                (2, "Я учусь в Московском университете.")
            ]
        );
    }

    #[test]
    fn word_times_move_together_with_the_text() {
        let mut t = transcript(&[(1, 0.0, 3.0, "Вы учитесь? Я"), (2, 3.2, 5.0, "учусь")]);
        t.turns[0].words = Some(vec![
            word("Вы", 0.0, 0.5),
            word("учитесь?", 0.5, 2.0),
            word("Я", 2.8, 3.0),
        ]);
        t.turns[1].words = Some(vec![word("учусь", 3.2, 5.0)]);
        let f = fix_split_sentences(t.turns);
        assert_eq!(shape(&f), [(1, "Вы учитесь?"), (2, "Я учусь")]);
        assert_eq!(f[0].end, 2.0);
        assert_eq!(f[1].start, 2.8);
        assert_eq!(f[0].words.as_ref().unwrap().len(), 2);
        assert_eq!(f[1].words.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn mismatching_word_times_are_dropped_not_misaligned() {
        let mut t = transcript(&[(1, 0.0, 3.0, "Вы учитесь? Я"), (2, 3.2, 5.0, "учусь")]);
        t.turns[0].words = Some(vec![word("Вы", 0.0, 0.5)]);
        let f = fix_split_sentences(t.turns);
        assert_eq!(shape(&f), [(1, "Вы учитесь?"), (2, "Я учусь")]);
        assert!(f[0].words.is_none() && f[1].words.is_none());
    }

    #[test]
    fn normal_dialog_is_left_alone() {
        let cases = [
            // Фраза закончена.
            vec![(1, 0.0, 1.0, "Как дела?"), (2, 1.1, 2.0, "хорошо.")],
            // Следующая реплика с заглавной буквы.
            vec![(1, 0.0, 1.0, "Вы учитесь? Я"), (2, 1.1, 2.0, "Учусь.")],
            // Долгая пауза.
            vec![(1, 0.0, 1.0, "Вы учитесь? Я"), (2, 5.0, 6.0, "учусь.")],
            // Хвост длиннее двух слов.
            vec![
                (1, 0.0, 1.0, "Да. Я думаю что нет"),
                (2, 1.1, 2.0, "ладно."),
            ],
            // Тот же спикер.
            vec![(1, 0.0, 1.0, "Вы учитесь? Я"), (1, 1.1, 2.0, "учусь.")],
        ];
        for case in cases {
            let t = transcript(&case);
            let before = shape(&t.turns)
                .into_iter()
                .map(|(s, x)| (s, x.to_string()))
                .collect::<Vec<_>>();
            let fixed = fix_split_sentences(t.turns);
            let after: Vec<(u32, String)> =
                fixed.iter().map(|t| (t.speaker, t.text.clone())).collect();
            assert_eq!(before, after, "{case:?}");
        }
    }

    #[test]
    fn stray_mark_is_added_when_the_previous_turn_has_none() {
        let t = transcript(&[(1, 0.0, 1.0, "Привет"), (2, 1.0, 1.2, "?")]);
        assert_eq!(shape(&fix_split_sentences(t.turns)), [(1, "Привет?")]);
    }

    #[test]
    fn turns_with_flags_are_not_touched() {
        let mut t = transcript(&[(1, 0.0, 1.0, "Вы учитесь? Я"), (2, 1.1, 2.0, "учусь.")]);
        t.turns[1].flags = vec!["overlap".into()];
        let f = fix_split_sentences(t.turns);
        assert_eq!(shape(&f), [(1, "Вы учитесь? Я"), (2, "учусь.")]);
    }

    #[test]
    fn empty_overlap_turn_stays_and_other_empty_turns_go() {
        let mut t = transcript(&[
            (1, 0.0, 3.0, "Привет."),
            (2, 1.0, 2.0, ""),
            (3, 4.0, 5.0, " "),
        ]);
        t.turns[1].flags = vec![OVERLAP_FLAG.into()];
        let f = fix_split_sentences(t.turns);
        assert_eq!(shape(&f), [(1, "Привет."), (2, "")]);
    }
}
