use nixyon_core::{EngineError, Pool, VoiceEmbedder};
use sherpa_onnx::SpeakerEmbeddingExtractor;

use crate::config::SAMPLE_RATE;

/// Модель голоса sherpa-onnx (роль embedding в models.json). Экстракторов может быть
/// несколько: что параллельные вызовы одного экстрактора безопасны, sherpa-onnx не
/// обещает, поэтому каждый поток считает в своём.
pub struct SherpaVoice {
    extractors: Pool<SpeakerEmbeddingExtractor>,
    dim: usize,
}

impl SherpaVoice {
    pub fn new(extractor: SpeakerEmbeddingExtractor) -> Self {
        let dim = usize::try_from(extractor.dim()).unwrap_or(0);
        Self {
            extractors: Pool::single(extractor),
            dim,
        }
    }

    /// Первый экстрактор и остальные, все одной модели.
    pub fn pooled(first: SpeakerEmbeddingExtractor, rest: Vec<SpeakerEmbeddingExtractor>) -> Self {
        let dim = usize::try_from(first.dim()).unwrap_or(0);
        Self {
            extractors: Pool::with_first(first, rest),
            dim,
        }
    }
}

impl VoiceEmbedder for SherpaVoice {
    fn embed(&self, samples_16k: &[f32]) -> Result<Vec<f32>, EngineError> {
        self.extractors
            .with(|extractor| {
                let stream = extractor
                    .create_stream()
                    .ok_or_else(|| failed("sherpa-onnx не создал поток модели голоса"))?;
                stream.accept_waveform(SAMPLE_RATE, samples_16k);
                stream.input_finished();
                if !extractor.is_ready(&stream) {
                    // Кусок слишком короткий для модели голоса.
                    return Ok(Vec::new());
                }
                extractor
                    .compute(&stream)
                    .ok_or_else(|| failed("sherpa-onnx не посчитал эмбеддинг голоса"))
            })
            .map_err(|e| failed(&e.to_string()))?
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

fn failed(message: &str) -> EngineError {
    EngineError::Failed {
        code: "engine_failed".into(),
        message: message.into(),
    }
}
