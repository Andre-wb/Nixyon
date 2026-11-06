use std::path::Path;

use nixyon_core::{Pool, WindowSpec};
use ort::session::Session;
use ort::value::Tensor;

use crate::runtime::{session, session_from_bytes};
use crate::weighted_vector_mean::weighted_vector_mean;
use crate::{OrtError, VoiceAnalysis, VoiceTraits};

const SAMPLE_RATE: u32 = 16_000;
const INPUT_NAME: &str = "signal";
const AGE_OUTPUT: &str = "logits_age";
const GENDER_OUTPUT: &str = "logits_gender";
const HIDDEN_OUTPUT: &str = "hidden_states";
/// Длина `hidden_states` у этой модели: выход уже усреднён по времени, форма [1, 1024].
pub(crate) const HIDDEN_DIM: usize = 1024;
const GENDER_CLASSES: usize = 3;
/// Кусок короче не оценивается: сверточный вход wav2vec2 даёт первый кадр только с 400
/// сэмплов (25 мс).
const MIN_SAMPLES: usize = 400;

/// Пол и возраст по голосу: audEERING wav2vec2-large-robust, 6 слоёв, официальная выгрузка
/// ONNX. Нормировка входа (нулевое среднее, единичная дисперсия) уже внутри графа: выход
/// не меняется ни от масштаба, ни от сдвига звука (проверено на модели). Кусок не длиннее
/// окна идёт одним вызовом, длиннее режется на окна (последнее прижато к концу), оценки
/// окон усредняются. Сессий может быть несколько: вызовы с разных потоков идут параллельно.
pub struct AgeGender {
    sessions: Pool<Session>,
    window: WindowSpec,
}

impl AgeGender {
    /// `sessions` сессий по `threads` потоков, окно `window_sec` секунд.
    pub fn load_pool(
        model: &Path,
        threads: usize,
        sessions: usize,
        window_sec: f64,
    ) -> Result<Self, OrtError> {
        Self::build(|| session(model, threads), sessions, window_sec)
    }

    /// Модель из байтов файла, одна сессия на `threads` потоков: так модель грузится из
    /// APK.
    pub fn from_bytes(model: &[u8], threads: usize, window_sec: f64) -> Result<Self, OrtError> {
        Self::build(|| session_from_bytes(model, threads), 1, window_sec)
    }

    fn build(
        open: impl Fn() -> Result<Session, OrtError>,
        sessions: usize,
        window_sec: f64,
    ) -> Result<Self, OrtError> {
        let window = WindowSpec::new(window_sec, window_sec)
            .ok_or_else(|| OrtError::BadOutput(format!("окно {window_sec} с")))?;
        let first = open()?;
        check_outputs(&first)?;
        let mut rest = Vec::new();
        for _ in 1..sessions.max(1) {
            rest.push(open()?);
        }
        Ok(Self {
            sessions: Pool::with_first(first, rest),
            window,
        })
    }

    /// Оценка куска звука 16 кГц; None, если кусок слишком короткий.
    pub fn traits(&self, samples_16k: &[f32]) -> Result<Option<VoiceTraits>, OrtError> {
        Ok(self.analyze(samples_16k)?.map(|a| a.traits))
    }

    /// Оценки и `hidden_states` куска звука 16 кГц, по окнам усреднённые с весом по длине;
    /// None, если кусок слишком короткий.
    pub fn analyze(&self, samples_16k: &[f32]) -> Result<Option<VoiceAnalysis>, OrtError> {
        if samples_16k.len() < MIN_SAMPLES {
            return Ok(None);
        }
        let spans = self.window.spans(samples_16k.len(), SAMPLE_RATE);
        if spans.is_empty() {
            return self.run(samples_16k.to_vec()).map(Some);
        }
        let mut traits = Vec::with_capacity(spans.len());
        let mut hidden = Vec::with_capacity(spans.len());
        for (a, b) in spans {
            let window = self.run(samples_16k[a..b].to_vec())?;
            let weight = (b - a) as f64;
            traits.push((window.traits, weight));
            hidden.push((window.hidden_states, weight));
        }
        Ok(VoiceTraits::weighted_mean(&traits)
            .zip(weighted_vector_mean(&hidden))
            .map(|(traits, hidden_states)| VoiceAnalysis {
                traits,
                hidden_states,
            }))
    }

    fn run(&self, window: Vec<f32>) -> Result<VoiceAnalysis, OrtError> {
        let input = Tensor::from_array(([1usize, window.len()], window))?;
        let (age, gender, hidden) = self
            .sessions
            .with(|session| -> Result<RawOutputs, OrtError> {
                let outputs = session.run(ort::inputs![INPUT_NAME => input])?;
                let (_, age) = outputs[AGE_OUTPUT].try_extract_tensor::<f32>()?;
                let (_, gender) = outputs[GENDER_OUTPUT].try_extract_tensor::<f32>()?;
                let (_, hidden) = outputs[HIDDEN_OUTPUT].try_extract_tensor::<f32>()?;
                Ok((age.to_vec(), gender.to_vec(), hidden.to_vec()))
            })
            .map_err(|_| OrtError::Poisoned)??;
        let gender: [f32; GENDER_CLASSES] = gender
            .try_into()
            .map_err(|v: Vec<f32>| OrtError::BadOutput(format!("классов пола {}", v.len())))?;
        if hidden.len() != HIDDEN_DIM || !hidden.iter().all(|x| x.is_finite()) {
            return Err(OrtError::BadOutput(format!(
                "{HIDDEN_OUTPUT} длины {}, ждали {HIDDEN_DIM} конечных чисел",
                hidden.len()
            )));
        }
        match age.as_slice() {
            [age] if age.is_finite() && gender.iter().all(|x| x.is_finite()) => Ok(VoiceAnalysis {
                traits: VoiceTraits::from_logits(*age, gender),
                hidden_states: hidden,
            }),
            _ => Err(OrtError::BadOutput(format!(
                "возраст {age:?}, пол {gender:?}"
            ))),
        }
    }
}

/// Выходы модели как есть: возраст, логиты пола, `hidden_states`.
type RawOutputs = (Vec<f32>, Vec<f32>, Vec<f32>);

fn check_outputs(session: &Session) -> Result<(), OrtError> {
    let has = |name: &str| session.outputs().iter().any(|o| o.name() == name);
    let has_input = session.inputs().iter().any(|i| i.name() == INPUT_NAME);
    if has_input && has(AGE_OUTPUT) && has(GENDER_OUTPUT) && has(HIDDEN_OUTPUT) {
        Ok(())
    } else {
        Err(OrtError::BadOutput(format!(
            "ждали вход {INPUT_NAME} и выходы {AGE_OUTPUT}, {GENDER_OUTPUT}, {HIDDEN_OUTPUT}"
        )))
    }
}
