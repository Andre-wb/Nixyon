use std::path::Path;

use nixyon_core::{
    CancelFlag, EngineError, NEURAL_BLOCK_SEC, SegmentResult, SpeechEngine, SpeechLanguage,
    VoiceSegmenter, VoiceSpan,
};
use nixyon_pipeline::PipelineOptions;

use crate::SherpaEngine;

/// Разбор WAV 16 кГц под настройки конвейера: куски речи на языке `language` с тем, что им
/// нужно, и, если включена нейросегментация, её отрезки по кускам VAD блоками по
/// `block_sec` секунд.
pub fn analyze_recording(
    engine: &SherpaEngine,
    wav_16k: &Path,
    options: &PipelineOptions,
    language: SpeechLanguage,
    block_sec: f64,
    cancel: &CancelFlag,
) -> Result<(Vec<SegmentResult>, Vec<VoiceSpan>), EngineError> {
    let segments = engine.analyze(wav_16k, &options.engine_options(language), cancel)?;
    if !options.neural_segmentation {
        return Ok((segments, Vec::new()));
    }
    let speech: Vec<(f64, f64)> = segments.iter().map(|s| (s.start, s.end)).collect();
    let spans = engine.voice_spans(wav_16k, &speech, block_sec, cancel)?;
    Ok((segments, spans))
}

/// То же с блоком по умолчанию, как в приложении.
pub fn analyze_default(
    engine: &SherpaEngine,
    wav_16k: &Path,
    options: &PipelineOptions,
    language: SpeechLanguage,
    cancel: &CancelFlag,
) -> Result<(Vec<SegmentResult>, Vec<VoiceSpan>), EngineError> {
    analyze_recording(engine, wav_16k, options, language, NEURAL_BLOCK_SEC, cancel)
}
