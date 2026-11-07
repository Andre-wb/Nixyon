use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum SherpaError {
    #[error("не удалось прочитать {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("models.json не читается: {0}")]
    BadManifest(String),
    #[error("sherpa-onnx не создал {0}")]
    CreateFailed(&'static str),
    #[error("не удалось записать {path}: {reason}")]
    Write { path: PathBuf, reason: String },
    #[error("WAV не читается: {0}")]
    BadWave(PathBuf),
    #[error("нужна частота {expected} Гц, в файле {actual} Гц")]
    SampleRate { expected: i32, actual: i32 },
    #[error("в наборе моделей нет словаря BPE для горячих слов")]
    NoBpeVocab,
    #[error("горячие слова есть, а распознаватель загружен без поиска по лучам")]
    HotwordsNeedBeam,
    #[error("в наборе моделей нет распознавания звуков, а отсечение музыки включено")]
    NoAudioTagging,
    #[error("в наборе моделей нет модели нейросегментации")]
    NoSegmentationModel,
    #[error("длина блока нейросегментации {0} с: нужно конечное положительное число")]
    BadBlock(f64),
    #[error("нейросегментация не обработала блок с {offset} с: {reason}")]
    SegmentationFailed { offset: f64, reason: String },
    #[error("модель голоса: {0}")]
    Voice(String),
    #[error("определение языка: {0}")]
    Language(String),
    #[error("в образце голоса {0:.1} с речи, нужно больше")]
    SampleTooShort(f64),
    #[error("обработка отменена")]
    Cancelled,
    #[error("модель испорчена: поток упал во время её работы")]
    Poisoned,
}

impl From<nixyon_core::EngineError> for SherpaError {
    fn from(e: nixyon_core::EngineError) -> Self {
        match e {
            nixyon_core::EngineError::Cancelled => SherpaError::Cancelled,
            nixyon_core::EngineError::Failed { message, .. } => SherpaError::Voice(message),
        }
    }
}
