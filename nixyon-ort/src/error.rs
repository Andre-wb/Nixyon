use std::path::PathBuf;

use nixyon_core::EngineError;

#[derive(Debug, thiserror::Error)]
pub enum OrtError {
    #[error("не удалось прочитать {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("models.json набора ort не читается: {0}")]
    BadManifest(String),
    #[error("в наборе ort нет роли {0}")]
    NoRole(&'static str),
    #[error("ONNX Runtime: {0}")]
    Runtime(String),
    #[error("модель вернула неожиданный ответ: {0}")]
    BadOutput(String),
    #[error("запись не читается: {0}")]
    Audio(String),
    #[error("сессия модели недоступна после сбоя в другом потоке")]
    Poisoned,
}

impl<R> From<ort::Error<R>> for OrtError {
    fn from(e: ort::Error<R>) -> Self {
        OrtError::Runtime(e.to_string())
    }
}

impl From<OrtError> for EngineError {
    fn from(e: OrtError) -> Self {
        EngineError::Failed {
            code: "engine_failed".into(),
            message: e.to_string(),
        }
    }
}
