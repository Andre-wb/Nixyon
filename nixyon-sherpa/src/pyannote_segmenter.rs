use nixyon_core::{BlockSegmenter, EngineError};
use sherpa_onnx::OfflineSpeakerDiarization;

/// Нейросегментация pyannote через sherpa-onnx (роль segmentation в models.json).
pub struct PyannoteSegmenter {
    diarizer: OfflineSpeakerDiarization,
}

impl PyannoteSegmenter {
    pub fn new(diarizer: OfflineSpeakerDiarization) -> Self {
        Self { diarizer }
    }
}

impl BlockSegmenter for PyannoteSegmenter {
    fn speaker_spans(&self, block_16k: &[f32]) -> Result<Vec<(f64, f64)>, EngineError> {
        let result = self
            .diarizer
            .process(block_16k)
            .ok_or_else(|| EngineError::Failed {
                code: "engine_failed".into(),
                message: "sherpa-onnx не обработал блок нейросегментации".into(),
            })?;
        Ok(result
            .sort_by_start_time()
            .iter()
            .map(|s| (f64::from(s.start), f64::from(s.end)))
            .collect())
    }
}
