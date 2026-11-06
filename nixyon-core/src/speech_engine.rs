use std::path::Path;

use crate::{CancelFlag, EngineOptions, SegmentResult};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("обработка отменена")]
    Cancelled,
    /// code: короткий код для интерфейса, message: подробности для лога.
    #[error("{code}: {message}")]
    Failed { code: String, message: String },
}

/// Граница ядра и движка распознавания. Звук передаётся путём к WAV 16 кГц моно,
/// а не массивом: на Android движок живёт в Kotlin, и гонять сэмплы через мост дорого.
pub trait SpeechEngine {
    fn analyze(
        &self,
        wav_16k: &Path,
        options: &EngineOptions,
        cancel: &CancelFlag,
    ) -> Result<Vec<SegmentResult>, EngineError>;
}
