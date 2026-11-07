use crate::{EngineError, SpeechLanguage};

/// Определение языка по одному отрезку речи.
pub trait LanguageDetector: Send + Sync {
    /// `clip_16k`: моно 16 кГц. None: на отрезке ни русский, ни английский.
    fn language(&self, clip_16k: &[f32]) -> Result<Option<SpeechLanguage>, EngineError>;
}
