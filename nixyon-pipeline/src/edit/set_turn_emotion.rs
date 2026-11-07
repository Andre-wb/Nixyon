use nixyon_core::{Transcript, TurnEmotion};

use crate::EditError;

/// Ставит реплике эмоцию, выбранную вручную; None убирает её. При ошибке расшифровка не
/// меняется.
pub fn set_turn_emotion(
    transcript: &mut Transcript,
    id: u32,
    emotion: Option<TurnEmotion>,
) -> Result<(), EditError> {
    let turn = transcript
        .turns
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or(EditError::UnknownTurn(id))?;
    turn.emotion = emotion;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    #[test]
    fn the_chosen_emotion_replaces_the_one_from_the_model() {
        let mut t = transcript(&[(1, 0.0, 2.0, "привет"), (2, 2.0, 3.0, "да")]);
        t.turns[0].emotion = Some(TurnEmotion::Neutral);
        set_turn_emotion(&mut t, 1, Some(TurnEmotion::Surprise)).unwrap();
        assert_eq!(t.turns[0].emotion, Some(TurnEmotion::Surprise));
        assert_eq!(t.turns[1].emotion, None);
    }

    #[test]
    fn none_clears_the_emotion() {
        let mut t = transcript(&[(1, 0.0, 2.0, "привет")]);
        t.turns[0].emotion = Some(TurnEmotion::Anger);
        set_turn_emotion(&mut t, 1, None).unwrap();
        assert_eq!(t.turns[0].emotion, None);
    }

    #[test]
    fn an_unknown_turn_is_an_error_and_changes_nothing() {
        let mut t = transcript(&[(1, 0.0, 2.0, "привет")]);
        let before = t.clone();
        assert_eq!(
            set_turn_emotion(&mut t, 7, Some(TurnEmotion::Joy)),
            Err(EditError::UnknownTurn(7))
        );
        assert_eq!(t, before);
    }
}
