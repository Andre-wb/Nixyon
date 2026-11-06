use serde::{Deserialize, Serialize};

/// Ребёнок или взрослый по голосу спикера.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerAge {
    Child,
    Adult,
}

impl SpeakerAge {
    /// Ответ классификатора: Some(true) ребёнок, Some(false) взрослый, None без метки.
    pub fn from_verdict(child: Option<bool>) -> Option<Self> {
        child.map(|c| if c { Self::Child } else { Self::Adult })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verdict_becomes_an_age_and_no_verdict_no_age() {
        assert_eq!(
            SpeakerAge::from_verdict(Some(true)),
            Some(SpeakerAge::Child)
        );
        assert_eq!(
            SpeakerAge::from_verdict(Some(false)),
            Some(SpeakerAge::Adult)
        );
        assert_eq!(SpeakerAge::from_verdict(None), None);
    }

    #[test]
    fn ages_are_written_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&SpeakerAge::Child).unwrap(),
            "\"child\""
        );
        assert_eq!(
            serde_json::to_string(&SpeakerAge::Adult).unwrap(),
            "\"adult\""
        );
    }
}
