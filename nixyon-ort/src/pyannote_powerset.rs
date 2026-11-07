use std::path::Path;
use std::sync::Mutex;

use nixyon_core::{CancelFlag, POWERSET_CLASSES, PowersetWindow};
use ort::session::Session;
use ort::value::Tensor;

use crate::OrtError;
use crate::runtime::{session, session_from_bytes};
use crate::softmax::softmax;

const SAMPLE_RATE: f64 = 16_000.0;
/// Окно модели 10 с, сдвиг 1 с: как у встроенной диаризации sherpa-onnx
/// (`window_shift_ratio` 0,1). Длина окна, шаг кадра и поле зрения кадра записаны в
/// метаданных модели (`window_size`, `receptive_field_shift`, `receptive_field_size`).
const WINDOW_SAMPLES: usize = 160_000;
const WINDOW_SHIFT_SAMPLES: usize = 16_000;
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

    /// Окна по записи 16 кГц. Последнее окно дополняется тишиной, кадры после конца
    /// записи отбрасываются. Пустая запись: окон нет. `None`: обработку отменили.
    pub fn windows(
        &self,
        samples_16k: &[f32],
        cancel: &CancelFlag,
    ) -> Result<Option<Vec<PowersetWindow>>, OrtError> {
        let duration_sec = samples_16k.len() as f64 / SAMPLE_RATE;
        let mut windows = Vec::new();
        let mut start = 0;
        while start < samples_16k.len() {
            if cancel.is_cancelled() {
                return Ok(None);
            }
            let end = (start + WINDOW_SAMPLES).min(samples_16k.len());
            let mut input = samples_16k[start..end].to_vec();
            input.resize(WINDOW_SAMPLES, 0.0);
            let mut window = self.window(input, start)?;
            let inside = (0..window.frames.len())
                .take_while(|&i| window.frame_time(i) < duration_sec)
                .count();
            window.frames.truncate(inside);
            windows.push(window);
            if end == samples_16k.len() {
                break;
            }
            start += WINDOW_SHIFT_SAMPLES;
        }
        Ok(Some(windows))
    }

    fn window(&self, input: Vec<f32>, start: usize) -> Result<PowersetWindow, OrtError> {
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
        Ok(PowersetWindow {
            first_frame_sec: (start as f64 + RECEPTIVE_FIELD_SAMPLES / 2.0) / SAMPLE_RATE,
            frame_sec: FRAME_SHIFT_SAMPLES / SAMPLE_RATE,
            frames,
        })
    }
}
