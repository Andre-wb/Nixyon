use nixyon_core::{AudioInfo, EngineInfo, SpeechLanguage};

/// Сведения о записи, которые конвейер не вычисляет сам.
#[derive(Debug, Clone)]
pub struct TranscriptMeta {
    /// RFC 3339, UTC.
    pub created_at: String,
    pub audio: AudioInfo,
    pub engine: EngineInfo,
    /// Язык речи, на котором запись распознана.
    pub language: SpeechLanguage,
}
