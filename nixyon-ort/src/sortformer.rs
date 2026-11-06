use std::path::Path;
use std::sync::Mutex;

use nixyon_core::{BlockSegmenter, EngineError};
use ort::session::Session;
use ort::value::Tensor;

use crate::log_mel::LogMel;
use crate::runtime::session;
use crate::{Binarize, OrtError};

const SAMPLE_RATE: u32 = 16_000;
const N_MELS: usize = 128;
const SPEAKERS: usize = 4;
/// Кадр предсказаний: шаг признаков 10 мс, сжатый моделью в 8 раз.
const FRAME_SEC: f64 = 0.08;
const INPUT_NAME: &str = "processed_signal";
const OUTPUT_NAME: &str = "speaker_logits";

/// Нейросегментация Sortformer 4spk-v2 (выгрузка DimQ1, целиком, не потоком): log-mel
/// блока на входе, вероятности речи четырёх говорящих по кадрам 80 мс на выходе.
pub struct Sortformer {
    session: Mutex<Session>,
    mel: LogMel,
    binarize: Binarize,
}

impl Sortformer {
    pub fn load(model: &Path, threads: usize, binarize: Binarize) -> Result<Self, OrtError> {
        Ok(Self {
            session: Mutex::new(session(model, threads)?),
            mel: LogMel::new(N_MELS, SAMPLE_RATE),
            binarize,
        })
    }

    /// Вероятности по кадрам: кадры подряд, в каждом по одной на говорящего.
    pub fn probabilities(&self, block_16k: &[f32]) -> Result<Vec<f32>, OrtError> {
        let (frames, features) = self.mel.features(block_16k)?;
        let input = Tensor::from_array(([1usize, N_MELS, frames], features))?;
        let mut session = self.session.lock().map_err(|_| OrtError::Poisoned)?;
        let outputs = session.run(ort::inputs![INPUT_NAME => input])?;
        let (shape, values) = outputs[OUTPUT_NAME].try_extract_tensor::<f32>()?;
        if shape.len() != 3 || shape[2] != SPEAKERS as i64 {
            return Err(OrtError::BadOutput(format!("форма выхода {shape:?}")));
        }
        Ok(values.to_vec())
    }
}

impl BlockSegmenter for Sortformer {
    fn speaker_spans(&self, block_16k: &[f32]) -> Result<Vec<(f64, f64)>, EngineError> {
        if block_16k.is_empty() {
            return Ok(Vec::new());
        }
        let probs = self.probabilities(block_16k)?;
        let mut spans = self.binarize.spans(&probs, SPEAKERS, FRAME_SEC);
        spans.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        Ok(spans)
    }
}
