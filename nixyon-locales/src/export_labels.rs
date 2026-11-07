use nixyon_render::{EmotionLabels, Labels};

use crate::dictionary::Dictionary;
use crate::lang::Lang;
use crate::locale_error::LocaleError;

/// Подписи экспорта (TXT, Markdown, SRT) на языке интерфейса.
pub fn export_labels(lang: Lang) -> Result<Labels, LocaleError> {
    let d = Dictionary::load(lang)?;
    let text = |key: &str| d.get(key).map(str::to_string);
    Ok(Labels {
        title: text("export.title")?,
        recorded: text("export.recorded")?,
        duration: text("export.duration")?,
        participants: text("export.participants")?,
        participant: text("export.participant")?,
        talk_time: text("export.talk_time")?,
        share: text("export.share")?,
        percent: text("export.percent")?,
        speaker_default: text("export.speaker_default")?,
        speaker_unsure: text("export.speaker_unsure")?,
        unclear: text("export.unclear")?,
        emotions: EmotionLabels {
            joy: text("export.emotion_joy")?,
            sadness: text("export.emotion_sadness")?,
            anger: text("export.emotion_anger")?,
            fear: text("export.emotion_fear")?,
            surprise: text("export.emotion_surprise")?,
            disgust: text("export.emotion_disgust")?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_come_in_the_interface_language_with_working_placeholders() {
        let ru = export_labels(Lang::Ru).unwrap();
        assert_eq!(ru.speaker(3), "Спикер 3");
        assert_eq!(ru.percent(41), "41\u{a0}%");
        assert_eq!(ru.title, "Расшифровка");
        let en = export_labels(Lang::En).unwrap();
        assert_eq!(en.speaker(3), "Speaker 3");
        assert_eq!(en.percent(41), "41%");
        assert_eq!(ru.speaker_unsure("Спикер 2"), "Спикер 2 (?)");
        assert_eq!(ru.unclear, "[неразборчиво]");
        assert_eq!(en.unclear, "[inaudible]");
    }

    #[test]
    fn emotion_labels_come_in_the_interface_language() {
        let ru = export_labels(Lang::Ru).unwrap();
        assert_eq!(ru.emotions.anger, "гнев");
        assert_eq!(ru.emotions.joy, "радость");
        let en = export_labels(Lang::En).unwrap();
        assert_eq!(en.emotions.surprise, "surprise");
    }

    #[test]
    fn device_language_codes_map_to_known_languages() {
        assert_eq!(Lang::from_code("en-US"), Some(Lang::En));
        assert_eq!(Lang::from_code("RU_ru"), Some(Lang::Ru));
        assert_eq!(Lang::from_code("de-DE"), None);
        assert_eq!(Lang::from_code(""), None);
    }
}
