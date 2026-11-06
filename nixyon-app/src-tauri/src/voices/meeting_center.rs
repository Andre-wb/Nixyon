use serde::{Deserialize, Serialize};

/// Центр голоса человека с одной встречи. `meeting`: время создания расшифровки, по нему
/// повторное запоминание с той же встречи заменяет центр, а не добавляет второй.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingCenter {
    pub meeting: String,
    pub embedding: Vec<f32>,
    pub remembered_at: String,
    /// Имя файла модели голоса, которой посчитан центр. None: центр сохранён до пометки
    /// моделью, с какой он модели, неизвестно.
    #[serde(default)]
    pub voice_model: Option<String>,
}
