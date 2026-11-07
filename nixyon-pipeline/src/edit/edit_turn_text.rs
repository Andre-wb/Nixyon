use nixyon_core::{Transcript, UNCLEAR_FLAG};

use crate::{EditError, recompute_speaker_stats};

/// Предел длины текста реплики. Подобран с запасом под монолог в несколько минут, замером
/// не проверялся.
pub const MAX_TURN_TEXT_CHARS: usize = 20_000;

/// Заменяет текст реплики исправленным. Время слов у этой реплики убирается: оно было
/// посчитано для старого текста. Пометка неразборчивой речи снимается: текст теперь
/// написал человек. Тот же текст ничего не меняет. При ошибке расшифровка не меняется.
pub fn edit_turn_text(transcript: &mut Transcript, id: u32, text: &str) -> Result<(), EditError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(EditError::TextEmpty);
    }
    if text.chars().count() > MAX_TURN_TEXT_CHARS {
        return Err(EditError::TextTooLong {
            max: MAX_TURN_TEXT_CHARS,
        });
    }
    if text.chars().any(char::is_control) {
        return Err(EditError::TextHasControlChars);
    }
    let turn = transcript
        .turns
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or(EditError::UnknownTurn(id))?;
    if turn.text == text {
        return Ok(());
    }
    turn.text = text.to_string();
    turn.words = None;
    turn.flags.retain(|f| f != UNCLEAR_FLAG);
    recompute_speaker_stats(transcript);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{transcript, word};

    fn sample() -> Transcript {
        let mut t = transcript(&[(1, 0.0, 2.0, "привет мир"), (2, 2.0, 3.0, "да")]);
        t.turns[0].words = Some(vec![word("привет", 0.0, 1.0), word("мир", 1.0, 2.0)]);
        recompute_speaker_stats(&mut t);
        t
    }

    #[test]
    fn written_text_takes_off_the_unclear_mark_and_keeps_other_marks() {
        let mut t = transcript(&[(1, 0.0, 2.0, "")]);
        t.turns[0].flags = vec![UNCLEAR_FLAG.into(), "overlap".into()];
        edit_turn_text(&mut t, 1, "Угу, понятно").unwrap();
        assert_eq!(t.turns[0].text, "Угу, понятно");
        assert_eq!(t.turns[0].flags, ["overlap"]);
    }

    #[test]
    fn new_text_replaces_old_drops_word_times_and_updates_stats() {
        let mut t = sample();
        edit_turn_text(&mut t, 1, "  Привет, большой мир!  ").unwrap();
        assert_eq!(t.turns[0].text, "Привет, большой мир!");
        assert_eq!(t.turns[0].words, None);
        assert_eq!(t.speaker(1).unwrap().words, 3);
    }

    #[test]
    fn same_text_keeps_word_times() {
        let mut t = sample();
        edit_turn_text(&mut t, 1, " привет мир ").unwrap();
        assert!(t.turns[0].words.is_some());
    }

    #[test]
    fn bad_text_or_unknown_turn_is_an_error_and_nothing_changes() {
        let mut t = sample();
        let before = t.clone();
        assert_eq!(edit_turn_text(&mut t, 1, "   "), Err(EditError::TextEmpty));
        assert_eq!(
            edit_turn_text(&mut t, 1, "при\nвет"),
            Err(EditError::TextHasControlChars)
        );
        assert_eq!(
            edit_turn_text(&mut t, 1, "при\0вет"),
            Err(EditError::TextHasControlChars)
        );
        assert_eq!(
            edit_turn_text(&mut t, 9, "текст"),
            Err(EditError::UnknownTurn(9))
        );
        assert_eq!(t, before);
    }

    #[test]
    fn text_of_max_length_in_cyrillic_is_accepted_and_one_more_is_rejected() {
        let mut t = sample();
        let max = "ж".repeat(MAX_TURN_TEXT_CHARS);
        edit_turn_text(&mut t, 2, &max).unwrap();
        assert_eq!(
            edit_turn_text(&mut t, 2, &format!("{max}ж")),
            Err(EditError::TextTooLong {
                max: MAX_TURN_TEXT_CHARS
            })
        );
        assert_eq!(t.turns[1].text, max);
    }
}
