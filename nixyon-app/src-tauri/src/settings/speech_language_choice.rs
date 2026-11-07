use nixyon_core::SpeechLanguage;
use serde::{Deserialize, Serialize};

/// Язык речи записей: определять по записи или задан пользователем.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpeechLanguageChoice {
    /// Язык каждой записи определяется по ней самой.
    #[default]
    Auto,
    Ru,
    En,
}

impl SpeechLanguageChoice {
    /// Заданный язык; None: определять по записи.
    pub fn fixed(self) -> Option<SpeechLanguage> {
        match self {
            SpeechLanguageChoice::Auto => None,
            SpeechLanguageChoice::Ru => Some(SpeechLanguage::Ru),
            SpeechLanguageChoice::En => Some(SpeechLanguage::En),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_leaves_the_language_to_the_recording() {
        assert_eq!(SpeechLanguageChoice::Auto.fixed(), None);
        assert_eq!(SpeechLanguageChoice::Ru.fixed(), Some(SpeechLanguage::Ru));
        assert_eq!(SpeechLanguageChoice::En.fixed(), Some(SpeechLanguage::En));
    }
}
