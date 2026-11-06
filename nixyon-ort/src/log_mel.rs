use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};

use crate::OrtError;
use crate::mel_filterbank::mel_filterbank;

const N_FFT: usize = 512;
/// Окно 25 мс и шаг 10 мс при 16 кГц.
const WIN_LENGTH: usize = 400;
const HOP_LENGTH: usize = 160;
const PREEMPH: f32 = 0.97;
/// 2^-24: добавка перед логарифмом, как `log_zero_guard_value` в NeMo.
const LOG_GUARD: f32 = 5.960_464_5e-8;

/// Признаки log-mel как у `AudioToMelSpectrogramPreprocessor` из NeMo с настройками
/// Sortformer: предыскажение 0,97, окно Ханна 25 мс, шаг 10 мс, БПФ 512, мощность,
/// мел по Слейни, натуральный логарифм, без нормировки. Шум (dither) NeMo добавляет
/// только при обучении, здесь его нет.
pub struct LogMel {
    n_mels: usize,
    filterbank: Vec<Vec<f32>>,
    window: Vec<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
}

impl LogMel {
    pub fn new(n_mels: usize, sample_rate: u32) -> Self {
        // Окно Ханна симметричное (periodic=False), по центру кадра БПФ, как в torch.stft.
        let offset = (N_FFT - WIN_LENGTH) / 2;
        let n = (WIN_LENGTH - 1) as f64;
        let mut window = vec![0.0f32; N_FFT];
        for i in 0..WIN_LENGTH {
            window[offset + i] =
                (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n).cos()) as f32;
        }
        Self {
            n_mels,
            filterbank: mel_filterbank(N_FFT, n_mels, sample_rate),
            window,
            fft: RealFftPlanner::<f32>::new().plan_fft_forward(N_FFT),
        }
    }

    /// Признаки куска: (число кадров, значения по полосам подряд, у каждой полосы все
    /// кадры), то есть порядок [полосы, кадры].
    pub fn features(&self, samples: &[f32]) -> Result<(usize, Vec<f32>), OrtError> {
        let mut signal = Vec::with_capacity(samples.len() + N_FFT);
        // center=True: по половине кадра нулей с каждой стороны.
        signal.resize(N_FFT / 2, 0.0);
        let mut prev = 0.0f32;
        for (i, &s) in samples.iter().enumerate() {
            signal.push(if i == 0 { s } else { s - PREEMPH * prev });
            prev = s;
        }
        signal.resize(signal.len() + N_FFT / 2, 0.0);

        let frames = (signal.len() - N_FFT) / HOP_LENGTH + 1;
        let mut out = vec![0.0f32; self.n_mels * frames];
        let mut input = self.fft.make_input_vec();
        let mut spectrum = self.fft.make_output_vec();
        let mut scratch = self.fft.make_scratch_vec();
        let mut power = vec![0.0f32; N_FFT / 2 + 1];
        for frame in 0..frames {
            let start = frame * HOP_LENGTH;
            for (k, x) in input.iter_mut().enumerate() {
                *x = signal[start + k] * self.window[k];
            }
            self.fft
                .process_with_scratch(&mut input, &mut spectrum, &mut scratch)
                .map_err(|e| OrtError::Runtime(format!("БПФ: {e}")))?;
            for (p, c) in power.iter_mut().zip(&spectrum) {
                *p = c.norm_sqr();
            }
            for (m, band) in self.filterbank.iter().enumerate() {
                let energy: f32 = band.iter().zip(&power).map(|(w, p)| w * p).sum();
                out[m * frames + frame] = (energy + LOG_GUARD).ln();
            }
        }
        Ok((frames, out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_second_gives_101_frames_of_128_bands() {
        let mel = LogMel::new(128, 16_000);
        let (frames, values) = mel.features(&vec![0.0; 16_000]).unwrap();
        assert_eq!(frames, 101);
        assert_eq!(values.len(), 128 * 101);
    }

    #[test]
    fn silence_sits_at_the_log_guard() {
        let mel = LogMel::new(128, 16_000);
        let (_, values) = mel.features(&vec![0.0; 1600]).unwrap();
        assert!(values.iter().all(|&v| (v - LOG_GUARD.ln()).abs() < 1e-3));
    }

    #[test]
    fn tone_is_loudest_in_the_band_of_its_frequency() {
        let mel = LogMel::new(128, 16_000);
        let filterbank = mel_filterbank(N_FFT, 128, 16_000);
        // 1 кГц приходится на бин 32 БПФ.
        let tone: Vec<f32> = (0..16_000)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 16_000.0).sin())
            .collect();
        let (frames, values) = mel.features(&tone).unwrap();
        let mid = frames / 2;
        let loudest = (0..128)
            .max_by(|&a, &b| values[a * frames + mid].total_cmp(&values[b * frames + mid]))
            .unwrap();
        let peak_bin = (0..257)
            .max_by(|&a, &b| filterbank[loudest][a].total_cmp(&filterbank[loudest][b]))
            .unwrap();
        assert!(
            peak_bin.abs_diff(32) <= 1,
            "полоса {loudest}, пик на бине {peak_bin}"
        );
    }
}
