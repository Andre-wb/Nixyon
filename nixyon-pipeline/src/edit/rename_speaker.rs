use nixyon_core::Transcript;

use crate::EditError;

const MAX_NAME_CHARS: usize = 64;

/// Даёт спикеру имя. Пустое имя (или из одних пробелов) возвращает подпись «Спикер N».
/// При ошибке расшифровка не меняется.
pub fn rename_speaker(transcript: &mut Transcript, id: u32, name: &str) -> Result<(), EditError> {
    let name = name.trim();
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(EditError::NameTooLong {
            max: MAX_NAME_CHARS,
        });
    }
    if name.chars().any(char::is_control) {
        return Err(EditError::NameHasControlChars);
    }
    let speaker = transcript
        .speaker_mut(id)
        .ok_or(EditError::UnknownSpeaker(id))?;
    speaker.name = if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    #[test]
    fn sets_trimmed_name() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а")]);
        rename_speaker(&mut t, 1, "  Анна  ").unwrap();
        assert_eq!(t.speaker(1).unwrap().name.as_deref(), Some("Анна"));
    }

    #[test]
    fn empty_or_blank_name_resets_to_default_label() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а")]);
        rename_speaker(&mut t, 1, "Анна").unwrap();
        rename_speaker(&mut t, 1, "   ").unwrap();
        assert_eq!(t.speaker(1).unwrap().name, None);
    }

    #[test]
    fn name_of_max_length_in_cyrillic_is_accepted_and_one_more_is_rejected() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а")]);
        let max = "Ж".repeat(MAX_NAME_CHARS);
        rename_speaker(&mut t, 1, &max).unwrap();
        let err = rename_speaker(&mut t, 1, &format!("{max}Ж")).unwrap_err();
        assert_eq!(
            err,
            EditError::NameTooLong {
                max: MAX_NAME_CHARS
            }
        );
        assert_eq!(t.speaker(1).unwrap().name.as_deref(), Some(max.as_str()));
    }

    #[test]
    fn control_chars_and_nul_are_rejected() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а")]);
        for bad in ["Ан\0на", "Ан\nна", "Ан\u{1b}на"] {
            assert_eq!(
                rename_speaker(&mut t, 1, bad),
                Err(EditError::NameHasControlChars)
            );
        }
        assert_eq!(t.speaker(1).unwrap().name, None);
    }

    #[test]
    fn unknown_speaker_is_an_error_and_nothing_changes() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а")]);
        let before = t.clone();
        assert_eq!(
            rename_speaker(&mut t, 7, "Анна"),
            Err(EditError::UnknownSpeaker(7))
        );
        assert_eq!(t, before);
    }
}
