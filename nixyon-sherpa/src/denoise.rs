use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use nixyon_audio::WavWriter;
use sherpa_onnx::{
    OfflineSpeechDenoiser, OfflineSpeechDenoiserConfig, OfflineSpeechDenoiserGtcrnModelConfig,
    OfflineSpeechDenoiserModelConfig,
};

use crate::SherpaError;
use crate::config::{SAMPLE_RATE, threads};
use crate::read_wave::read_wave;

/// Убирает шум из WAV 16 кГц моделью GTCRN и пишет результат в WAV 16 кГц.
pub fn denoise_wav(model: &Path, input: &Path, output: &Path) -> Result<(), SherpaError> {
    let wave = read_wave(input)?;
    let denoiser = OfflineSpeechDenoiser::create(&OfflineSpeechDenoiserConfig {
        model: OfflineSpeechDenoiserModelConfig {
            gtcrn: OfflineSpeechDenoiserGtcrnModelConfig {
                model: Some(model.to_string_lossy().into_owned()),
            },
            num_threads: threads(),
            ..OfflineSpeechDenoiserModelConfig::default()
        },
    })
    .ok_or(SherpaError::CreateFailed("шумоподавитель"))?;
    let denoised = denoiser.run(wave.samples(), wave.sample_rate());
    if denoised.sample_rate != SAMPLE_RATE {
        return Err(SherpaError::SampleRate {
            expected: SAMPLE_RATE,
            actual: denoised.sample_rate,
        });
    }
    let write = |path: &Path| -> Result<(), nixyon_audio::AudioError> {
        let file = File::create(path)?;
        // Частота 16 кГц положительна, приведение без потерь.
        let rate = SAMPLE_RATE as u32;
        let mut writer = WavWriter::new(BufWriter::new(file), rate)?;
        writer.write(&denoised.samples)?;
        writer.finish(rate)?;
        Ok(())
    };
    write(output).map_err(|e| SherpaError::Write {
        path: output.to_path_buf(),
        reason: e.to_string(),
    })
}
