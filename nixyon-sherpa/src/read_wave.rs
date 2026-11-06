use std::path::Path;

use sherpa_onnx::Wave;

use crate::SherpaError;
use crate::config::SAMPLE_RATE;

/// Сэмплы WAV 16 кГц: чтобы распознать запись один раз и считать по ней эмбеддинги
/// разными моделями голоса.
pub fn read_samples(wav: &Path) -> Result<Vec<f32>, SherpaError> {
    Ok(read_wave(wav)?.samples().to_vec())
}

/// Читает WAV и проверяет, что он 16 кГц.
pub(crate) fn read_wave(wav: &Path) -> Result<Wave, SherpaError> {
    let wave = Wave::read(&wav.to_string_lossy())
        .ok_or_else(|| SherpaError::BadWave(wav.to_path_buf()))?;
    if wave.sample_rate() != SAMPLE_RATE {
        return Err(SherpaError::SampleRate {
            expected: SAMPLE_RATE,
            actual: wave.sample_rate(),
        });
    }
    Ok(wave)
}
