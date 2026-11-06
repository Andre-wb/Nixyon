use std::path::Path;

use nixyon_core::{EngineError, Pool, VoiceEmbedder, WindowSpec};
use ort::session::Session;
use ort::value::Tensor;

use crate::OrtError;
use crate::mean_unit::mean_unit;
use crate::redimnet_input::{FIXED_SAMPLES, RedimnetInput};
use crate::runtime::{session, session_from_bytes};

const SAMPLE_RATE: u32 = 16_000;
/// Кусок длиннее режется на окна этой длины, признаки модель считает сама.
const WINDOW_SEC: f64 = 6.0;
const INPUT_NAME: &str = "audio";
const OUTPUT_NAME: &str = "embedding";
/// Кусок короче не получает эмбеддинга. Столько же требует модель голоса sherpa-onnx
/// (замерено), поэтому при сравнении моделей набор кусков с эмбеддингом одинаковый.
const MIN_SAMPLES: usize = 400;

/// Модель голоса ReDimNet2, выход уже длины 1. Вход постоянный 6 с (выгрузка soniqo) или
/// любой длины (своя выгрузка): режим берётся из формы входа модели. Кусок до 6 с идёт
/// одним вызовом (при постоянном входе повторён до 6 с), длиннее режется на окна по 6 с
/// (последнее прижато к концу), эмбеддинги окон усредняются. Сессий может быть несколько:
/// тогда вызовы с разных потоков идут параллельно, каждый в своей сессии.
pub struct Redimnet2 {
    sessions: Pool<Session>,
    dim: usize,
    input: RedimnetInput,
}

impl Redimnet2 {
    pub fn load(model: &Path, threads: usize) -> Result<Self, OrtError> {
        Self::load_pool(model, threads, 1)
    }

    /// `sessions` сессий модели по `threads` потоков в каждой.
    pub fn load_pool(model: &Path, threads: usize, sessions: usize) -> Result<Self, OrtError> {
        Self::build(|| session(model, threads), sessions)
    }

    /// Модель из байтов файла, одна сессия на `threads` потоков.
    pub fn from_bytes(model: &[u8], threads: usize) -> Result<Self, OrtError> {
        Self::build(|| session_from_bytes(model, threads), 1)
    }

    fn build(
        open: impl Fn() -> Result<Session, OrtError>,
        sessions: usize,
    ) -> Result<Self, OrtError> {
        let session = open()?;
        let dim = session
            .outputs()
            .iter()
            .find(|o| o.name() == OUTPUT_NAME)
            .and_then(|o| o.dtype().tensor_shape().and_then(|s| s.last().copied()))
            .and_then(|d| usize::try_from(d).ok())
            .ok_or_else(|| OrtError::BadOutput(format!("нет выхода {OUTPUT_NAME} с длиной")))?;
        let input = session
            .inputs()
            .iter()
            .find(|i| i.name() == INPUT_NAME)
            .and_then(|i| i.dtype().tensor_shape().and_then(|s| s.last().copied()))
            .and_then(RedimnetInput::from_dim)
            .ok_or_else(|| {
                OrtError::BadOutput(format!("вход {INPUT_NAME} не 6 с и не любой длины"))
            })?;
        let mut rest = Vec::new();
        for _ in 1..sessions.max(1) {
            rest.push(open()?);
        }
        Ok(Self {
            sessions: Pool::with_first(session, rest),
            dim,
            input,
        })
    }

    fn run(&self, window: Vec<f32>) -> Result<Vec<f32>, OrtError> {
        let input = Tensor::from_array(([1usize, window.len()], window))?;
        let values = self
            .sessions
            .with(|session| -> Result<Vec<f32>, OrtError> {
                let outputs = session.run(ort::inputs![INPUT_NAME => input])?;
                let (_, values) = outputs[OUTPUT_NAME].try_extract_tensor::<f32>()?;
                Ok(values.to_vec())
            })
            .map_err(|_| OrtError::Poisoned)??;
        if values.len() != self.dim {
            return Err(OrtError::BadOutput(format!(
                "эмбеддинг длины {}, ждали {}",
                values.len(),
                self.dim
            )));
        }
        Ok(values)
    }

    fn embed_samples(&self, samples: &[f32]) -> Result<Vec<f32>, OrtError> {
        if samples.len() < MIN_SAMPLES {
            return Ok(Vec::new());
        }
        if samples.len() <= FIXED_SAMPLES {
            return self.run(self.input.piece(samples));
        }
        let windows = WindowSpec::new(WINDOW_SEC, WINDOW_SEC)
            .ok_or_else(|| OrtError::BadOutput("окно модели голоса".into()))?
            .spans(samples.len(), SAMPLE_RATE);
        let embeddings = windows
            .into_iter()
            .map(|(a, b)| self.run(samples[a..b].to_vec()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(mean_unit(&embeddings))
    }
}

impl VoiceEmbedder for Redimnet2 {
    fn embed(&self, samples_16k: &[f32]) -> Result<Vec<f32>, EngineError> {
        Ok(self.embed_samples(samples_16k)?)
    }

    fn dim(&self) -> usize {
        self.dim
    }
}
