use std::path::Path;
use std::sync::Mutex;

use nixyon_core::{POWERSET_CLASSES, PowersetWindow};
use ort::session::Session;
use ort::value::Tensor;

use crate::OrtError;
use crate::runtime::{session, session_from_bytes};
use crate::softmax::softmax;

pub(crate) const SAMPLE_RATE: f64 = 16_000.0;
/// Окно модели 10 с, сдвиг 1 с: как у встроенной диаризации sherpa-onnx
/// (`window_shift_ratio` 0,1). Длина окна, шаг кадра и поле зрения кадра записаны в
/// метаданных модели (`window_size`, `receptive_field_shift`, `receptive_field_size`).
pub(crate) const WINDOW_SAMPLES: usize = 160_000;
pub(crate) const WINDOW_SHIFT_SAMPLES: usize = 16_000;
const FRAME_SHIFT_SAMPLES: f64 = 270.0;
const RECEPTIVE_FIELD_SAMPLES: f64 = 991.0;
const INPUT_NAME: &str = "x";
const OUTPUT_NAME: &str = "y";

/// Нейросегментация pyannote segmentation-3.0 напрямую, без обработки sherpa-onnx:
/// вероятности классов по кадрам каждого окна. Нужна, чтобы видеть перекрытия, которые
/// sherpa теряет при сборке отрезков.
pub struct PyannotePowerset {
    session: Mutex<Session>,
}

impl PyannotePowerset {
    pub fn load(model: &Path, threads: usize) -> Result<Self, OrtError> {
        Ok(Self {
            session: Mutex::new(session(model, threads)?),
        })
    }

    /// Модель из байтов файла: так грузится модель, которая лежит внутри APK.
    pub fn from_bytes(model: &[u8], threads: usize) -> Result<Self, OrtError> {
        Ok(Self {
            session: Mutex::new(session_from_bytes(model, threads)?),
        })
    }

    /// Окно с отсчёта `start`: `input` ровно длины окна, из них `length` настоящих, дальше
    /// тишина. Кадры после настоящего звука отбрасываются.
    pub(crate) fn window(
        &self,
        input: Vec<f32>,
        start: usize,
        length: usize,
    ) -> Result<PowersetWindow, OrtError> {
        let tensor = Tensor::from_array(([1usize, 1, WINDOW_SAMPLES], input))?;
        let mut session = self.session.lock().map_err(|_| OrtError::Poisoned)?;
        let outputs = session.run(ort::inputs![INPUT_NAME => tensor])?;
        let (shape, values) = outputs[OUTPUT_NAME].try_extract_tensor::<f32>()?;
        if shape.len() != 3 || shape[2] != POWERSET_CLASSES as i64 {
            return Err(OrtError::BadOutput(format!("форма выхода {shape:?}")));
        }
        let frames = values
            .chunks_exact(POWERSET_CLASSES)
            .map(|c| {
                let mut logits = [0.0f32; POWERSET_CLASSES];
                logits.copy_from_slice(c);
                softmax(logits)
            })
            .collect();
        let mut window = PowersetWindow {
            first_frame_sec: (start as f64 + RECEPTIVE_FIELD_SAMPLES / 2.0) / SAMPLE_RATE,
            frame_sec: FRAME_SHIFT_SAMPLES / SAMPLE_RATE,
            frames,
        };
        let end_sec = (start + length) as f64 / SAMPLE_RATE;
        let inside = (0..window.frames.len())
            .take_while(|&i| window.frame_time(i) < end_sec)
            .count();
        window.frames.truncate(inside);
        Ok(window)
    }
}
