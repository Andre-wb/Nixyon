use serde::{Deserialize, Serialize};

use super::meeting_center::MeetingCenter;

/// Сохранённый голос участника: имя, эмбеддинг по образцу, когда он записан (RFC 3339,
/// UTC) и центры голоса с прошлых встреч. Звук не хранится. `default`: файлы без
/// новых полей тоже читаются.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceRecord {
    pub name: String,
    /// None: голос запомнен только по встречам, образца нет.
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
    /// Имя файла модели голоса, которой посчитан образец. None: образец записан до
    /// пометки моделью.
    #[serde(default)]
    pub voice_model: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub centers: Vec<MeetingCenter>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_saved_before_centers_appeared_is_read_with_its_sample() {
        let old = r#"{"name":"Анна","embedding":[0.5,-0.5],"created_at":"2026-10-29T10:00:00Z"}"#;
        let voice: VoiceRecord = serde_json::from_str(old).unwrap();
        assert_eq!(voice.embedding, Some(vec![0.5, -0.5]));
        assert_eq!(voice.voice_model, None);
        assert!(voice.centers.is_empty());
    }
}
