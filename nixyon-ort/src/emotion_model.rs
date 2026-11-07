use std::path::Path;

use nixyon_core::WindowSpec;
use ort::session::Session;
use ort::value::Tensor;

use crate::runtime::{session, session_from_bytes};
use crate::{EmotionProbs, OrtError};

const SAMPLE_RATE: u32 = 16_000;
const INPUT_NAME: &str = "audio";
const OUTPUT_NAME: &str = "probs";
const CLASSES: usize = 4;
/// Окно 8 с без перекрытия: реплики Dusha, на которых обучена модель, обычно короче,
/// а внимание по всей длинной реплике стоит памяти квадратично.
const WINDOW_SEC: f64 = 8.0;
/// Кусок короче 0,25 с не оценивается: после свёрток энкодера от него почти ничего не
/// остаётся, а такие реплики («да», «угу») по голосу всё равно не разобрать.
const MIN_SAMPLES: usize = 4_000;

/// Эмоция по голосу: GigaAM-Emo (SberDevices, MIT) в своей выгрузке ONNX
/// (`scripts/export-gigaam-emo.py`). Лог-мел признаки считаются внутри графа, на вход
/// подаётся звук 16 кГц. Длинный кусок режется на окна, последнее прижато к концу,
/// вероятности окон усредняются с весом по длине.
pub struct EmotionModel {
    session: std::sync::Mutex<Session>,
    window: WindowSpec,
}

impl EmotionModel {
    pub fn load(model: &Path, threads: usize) -> Result<Self, OrtError> {
        Self::build(session(model, threads)?)
    }

    /// Модель из байтов файла: так модель грузится из APK.
    pub fn from_bytes(model: &[u8], threads: usize) -> Result<Self, OrtError> {
        Self::build(session_from_bytes(model, threads)?)
    }

    fn build(session: Session) -> Result<Self, OrtError> {
        let has_input = session.inputs().iter().any(|i| i.name() == INPUT_NAME);
        let has_output = session.outputs().iter().any(|o| o.name() == OUTPUT_NAME);
        if !has_input || !has_output {
            return Err(OrtError::BadOutput(format!(
                "ждали вход {INPUT_NAME} и выход {OUTPUT_NAME}"
            )));
        }
        let window = WindowSpec::new(WINDOW_SEC, WINDOW_SEC)
            .ok_or_else(|| OrtError::BadOutput(format!("окно {WINDOW_SEC} с")))?;
        Ok(Self {
            session: std::sync::Mutex::new(session),
            window,
        })
    }

    /// Вероятности классов куска звука 16 кГц; None, если кусок слишком короткий.
    pub fn probs(&self, samples_16k: &[f32]) -> Result<Option<EmotionProbs>, OrtError> {
        if samples_16k.len() < MIN_SAMPLES {
            return Ok(None);
        }
        let spans = self.window.spans(samples_16k.len(), SAMPLE_RATE);
        if spans.is_empty() {
            return self.run(samples_16k.to_vec()).map(Some);
        }
        let mut windows = Vec::with_capacity(spans.len());
        for (a, b) in spans {
            windows.push((self.run(samples_16k[a..b].to_vec())?, (b - a) as f64));
        }
        Ok(EmotionProbs::weighted_mean(&windows))
    }

    fn run(&self, window: Vec<f32>) -> Result<EmotionProbs, OrtError> {
        let input = Tensor::from_array(([1usize, window.len()], window))?;
        let mut session = self.session.lock().map_err(|_| OrtError::Poisoned)?;
        let outputs = session.run(ort::inputs![INPUT_NAME => input])?;
        let (_, probs) = outputs[OUTPUT_NAME].try_extract_tensor::<f32>()?;
        let probs: [f32; CLASSES] = probs
            .to_vec()
            .try_into()
            .map_err(|v: Vec<f32>| OrtError::BadOutput(format!("классов эмоций {}", v.len())))?;
        if !probs.iter().all(|p| p.is_finite()) {
            return Err(OrtError::BadOutput(format!("вероятности {probs:?}")));
        }
        Ok(EmotionProbs::from_output(probs))
    }
}
