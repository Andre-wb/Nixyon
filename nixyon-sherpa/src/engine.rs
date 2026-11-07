use std::path::Path;

use nixyon_core::{
    CancelFlag, EngineError, EngineOptions, SegmentResult, SpeechEngine, VoiceSegmenter, VoiceSpan,
};

use crate::{SherpaError, SherpaModels, analyze, voice_spans};

/// SpeechEngine поверх sherpa-onnx на ноутбуке.
pub struct SherpaEngine {
    models: SherpaModels,
}

impl SherpaEngine {
    pub fn new(models: SherpaModels) -> Self {
        Self { models }
    }

    pub fn models(&self) -> &SherpaModels {
        &self.models
    }
}

impl SpeechEngine for SherpaEngine {
    fn analyze(
        &self,
        wav_16k: &Path,
        options: &EngineOptions,
        cancel: &CancelFlag,
    ) -> Result<Vec<SegmentResult>, EngineError> {
        analyze(&self.models, wav_16k, options, cancel).map_err(engine_error)
    }
}

impl VoiceSegmenter for SherpaEngine {
    fn voice_spans(
        &self,
        wav_16k: &Path,
        speech: &[(f64, f64)],
        block_sec: f64,
        cancel: &CancelFlag,
    ) -> Result<Vec<VoiceSpan>, EngineError> {
        voice_spans(&self.models, wav_16k, speech, block_sec, cancel).map_err(engine_error)
    }
}

fn engine_error(e: SherpaError) -> EngineError {
    match e {
        SherpaError::Cancelled => EngineError::Cancelled,
        other => EngineError::Failed {
            code: code(&other).into(),
            message: other.to_string(),
        },
    }
}

fn code(e: &SherpaError) -> &'static str {
    match e {
        SherpaError::Read { .. }
        | SherpaError::Write { .. }
        | SherpaError::BadManifest(_)
        | SherpaError::CreateFailed(_)
        | SherpaError::NoSegmentationModel
        | SherpaError::NoAudioTagging
        | SherpaError::NoBpeVocab
        | SherpaError::HotwordsNeedBeam
        | SherpaError::BadBlock(_)
        | SherpaError::SegmentationFailed { .. }
        | SherpaError::Voice(_)
        | SherpaError::Language(_)
        | SherpaError::Poisoned => "engine_failed",
        SherpaError::BadWave(_) | SherpaError::SampleRate { .. } => "bad_audio",
        SherpaError::SampleTooShort(_) => "sample_too_short",
        SherpaError::Cancelled => "cancelled",
    }
}
