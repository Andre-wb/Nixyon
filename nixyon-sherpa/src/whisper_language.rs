use nixyon_core::{EngineError, LanguageDetector, SpeechLanguage};
use sherpa_onnx::SpokenLanguageIdentification;

use crate::config::{self, SAMPLE_RATE};
use crate::{ModelManifest, SherpaError};

/// Определение языка Whisper tiny (роли lid_* в models.json).
pub struct WhisperLanguage {
    model: SpokenLanguageIdentification,
}

impl WhisperLanguage {
    pub fn load(manifest: &ModelManifest) -> Result<Self, SherpaError> {
        SpokenLanguageIdentification::create(&config::language_id(manifest))
            .map(|model| Self { model })
            .ok_or(SherpaError::CreateFailed("определение языка"))
    }
}

impl LanguageDetector for WhisperLanguage {
    fn language(&self, clip_16k: &[f32]) -> Result<Option<SpeechLanguage>, EngineError> {
        let stream = self.model.create_stream();
        stream.accept_waveform(SAMPLE_RATE, clip_16k);
        let result = self
            .model
            .compute(&stream)
            .ok_or_else(|| EngineError::Failed {
                code: "engine_failed".into(),
                message: "определение языка не дало ответа".into(),
            })?;
        Ok(SpeechLanguage::from_code(&result.lang))
    }
}
