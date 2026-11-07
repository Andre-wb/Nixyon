use std::path::Path;

use nixyon_core::{CancelFlag, EngineError, EngineOptions, SpeechLanguage};

use crate::read_wave::read_wave;
use crate::{SherpaError, SherpaModels, speech_bounds};

/// Язык записи 16 кГц: Whisper слушает до пяти кусков речи из разных частей записи, решает
/// большинство (`nixyon_core::detect_language`). Куски речи те же, что потом распознаются
/// с `options`.
pub fn detect_language(
    models: &SherpaModels,
    samples: &[f32],
    options: &EngineOptions,
    cancel: &CancelFlag,
) -> Result<SpeechLanguage, SherpaError> {
    let bounds = speech_bounds(models, samples, options)?;
    nixyon_core::detect_language(models.language_detector(), samples, &bounds, cancel).map_err(
        |e| match e {
            EngineError::Cancelled => SherpaError::Cancelled,
            EngineError::Failed { message, .. } => SherpaError::Language(message),
        },
    )
}

/// То же по пути к WAV 16 кГц.
pub fn detect_wav_language(
    models: &SherpaModels,
    wav: &Path,
    options: &EngineOptions,
    cancel: &CancelFlag,
) -> Result<SpeechLanguage, SherpaError> {
    let wave = read_wave(wav)?;
    detect_language(models, wave.samples(), options, cancel)
}
