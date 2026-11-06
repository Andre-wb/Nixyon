use serde::{Deserialize, Serialize};

/// Пол по голосу спикера.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerGender {
    Female,
    Male,
}

impl SpeakerGender {
    /// Ответ модели: Some(true) женский голос, Some(false) мужской, None без метки.
    pub fn from_female(female: Option<bool>) -> Option<Self> {
        female.map(|f| if f { Self::Female } else { Self::Male })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_becomes_a_gender_and_no_answer_no_gender() {
        assert_eq!(
            SpeakerGender::from_female(Some(true)),
            Some(SpeakerGender::Female)
        );
        assert_eq!(
            SpeakerGender::from_female(Some(false)),
            Some(SpeakerGender::Male)
        );
        assert_eq!(SpeakerGender::from_female(None), None);
    }

    #[test]
    fn genders_are_written_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&SpeakerGender::Female).unwrap(),
            "\"female\""
        );
        assert_eq!(
            serde_json::to_string(&SpeakerGender::Male).unwrap(),
            "\"male\""
        );
    }
}
