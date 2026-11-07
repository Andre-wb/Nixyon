use nixyon_core::Transcript;

use crate::{EditError, recompute_speaker_stats};

/// Удаляет реплику целиком. Спикер, у которого не осталось реплик, пропадает из списка.
/// Последнюю реплику удалить нельзя: пустая расшифровка не нужна. При ошибке расшифровка
/// не меняется.
pub fn delete_turn(transcript: &mut Transcript, id: u32) -> Result<(), EditError> {
    let index = transcript
        .turns
        .iter()
        .position(|t| t.id == id)
        .ok_or(EditError::UnknownTurn(id))?;
    if transcript.turns.len() == 1 {
        return Err(EditError::LastTurn);
    }
    transcript.turns.remove(index);
    let speakers: Vec<u32> = transcript.turns.iter().map(|t| t.speaker).collect();
    transcript.speakers.retain(|s| speakers.contains(&s.id));
    recompute_speaker_stats(transcript);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    #[test]
    fn removes_the_turn_and_speakers_left_without_turns() {
        let mut t = transcript(&[
            (1, 0.0, 2.0, "привет мир"),
            (2, 2.0, 3.0, "да"),
            (1, 3.0, 4.0, "пока"),
        ]);
        delete_turn(&mut t, 2).unwrap();
        assert_eq!(t.turns.len(), 2);
        assert_eq!(t.speakers.len(), 1);
        assert_eq!(t.speaker(1).unwrap().turns, 2);
        assert_eq!(t.speaker(1).unwrap().words, 3);
    }

    #[test]
    fn unknown_or_last_turn_is_an_error_and_nothing_changes() {
        let mut t = transcript(&[(1, 0.0, 2.0, "привет"), (2, 2.0, 3.0, "да")]);
        let before = t.clone();
        assert_eq!(delete_turn(&mut t, 9), Err(EditError::UnknownTurn(9)));
        assert_eq!(t, before);
        delete_turn(&mut t, 1).unwrap();
        assert_eq!(delete_turn(&mut t, 2), Err(EditError::LastTurn));
        assert_eq!(t.turns.len(), 1);
    }
}
