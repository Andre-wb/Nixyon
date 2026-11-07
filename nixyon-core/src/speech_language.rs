use serde::{Deserialize, Serialize};

/// Язык речи записи. От него зависит модель распознавания: русский распознаёт GigaAM,
/// английский Parakeet. Расшифровки без этого поля записаны до английского, они русские.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SpeechLanguage {
    #[default]
    Ru,
    En,
}

impl SpeechLanguage {
    /// Код языка ISO 639-1, как его отдаёт определение языка Whisper.
    pub fn code(self) -> &'static str {
        match self {
            SpeechLanguage::Ru => "ru",
            SpeechLanguage::En => "en",
        }
    }

    /// Язык по коду ISO 639-1; другой язык даёт None.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "ru" => Some(SpeechLanguage::Ru),
            "en" => Some(SpeechLanguage::En),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_round_trips_and_other_languages_are_none() {
        for lang in [SpeechLanguage::Ru, SpeechLanguage::En] {
            assert_eq!(SpeechLanguage::from_code(lang.code()), Some(lang));
        }
        assert_eq!(SpeechLanguage::from_code("de"), None);
        assert_eq!(SpeechLanguage::from_code(""), None);
    }

    #[test]
    fn serialized_as_lowercase_code() {
        assert_eq!(
            serde_json::to_string(&SpeechLanguage::En).unwrap(),
            "\"en\""
        );
        let ru: SpeechLanguage = serde_json::from_str("\"ru\"").unwrap();
        assert_eq!(ru, SpeechLanguage::Ru);
    }
}
