use std::path::Path;

use sherpa_onnx::{
    OfflineSpeechDenoiserGtcrnModelConfig, OfflineSpeechDenoiserModelConfig, OnlineSpeechDenoiser,
    OnlineSpeechDenoiserConfig, VadModelConfig,
};

use crate::SherpaError;
use crate::config::SAMPLE_RATE;
use crate::models::SherpaModels;
use crate::speech_regions::{CHUNK, drain};

// GTCRN считает кадр за кадром, потоки ему почти не помогают; так же в SpeechModels.kt.
const DENOISER_THREADS: i32 = 1;

/// Шумоподавитель GTCRN для второго VAD, как в Kotlin-плагине (`SpeechFinder`): VAD
/// слушает звук после потокового шумоподавления, а его куски заменяют только длинные
/// куски VAD по исходному звуку. Распознавание и голоса слушают исходную запись.
pub struct VadDenoiser {
    config: OnlineSpeechDenoiserConfig,
}

impl VadDenoiser {
    /// Пробная загрузка: ошибка в модели видна сразу, а не на первой записи.
    pub fn load(model: &Path) -> Result<Self, SherpaError> {
        let config = OnlineSpeechDenoiserConfig {
            model: OfflineSpeechDenoiserModelConfig {
                gtcrn: OfflineSpeechDenoiserGtcrnModelConfig {
                    model: Some(model.to_string_lossy().into_owned()),
                },
                num_threads: DENOISER_THREADS,
                ..OfflineSpeechDenoiserModelConfig::default()
            },
        };
        OnlineSpeechDenoiser::create(&config).ok_or(SherpaError::CreateFailed("шумоподавитель"))?;
        Ok(Self { config })
    }

    /// Куски речи VAD по звуку после шумоподавления, в сэмплах исходной записи. Звук
    /// подаётся по 0,1 с, хвост шумоподавителя приходит с flush.
    pub(crate) fn clean_bounds(
        &self,
        vad_config: &VadModelConfig,
        samples: &[f32],
    ) -> Result<Vec<(usize, usize)>, SherpaError> {
        let denoiser = OnlineSpeechDenoiser::create(&self.config)
            .ok_or(SherpaError::CreateFailed("шумоподавитель"))?;
        let vad = SherpaModels::new_vad(vad_config)?;
        let mut regions = Vec::new();
        let mut hear = |audio: sherpa_onnx::DenoisedAudio| -> Result<(), SherpaError> {
            if audio.sample_rate != SAMPLE_RATE {
                return Err(SherpaError::SampleRate {
                    expected: SAMPLE_RATE,
                    actual: audio.sample_rate,
                });
            }
            if !audio.samples.is_empty() {
                vad.accept_waveform(&audio.samples);
            }
            drain(&vad, &mut regions);
            Ok(())
        };
        for chunk in samples.chunks(CHUNK) {
            hear(denoiser.run(chunk, SAMPLE_RATE))?;
        }
        hear(denoiser.flush())?;
        vad.flush();
        drain(&vad, &mut regions);
        Ok(regions
            .iter()
            .map(|r| {
                let start = usize::try_from(r.start_sample).unwrap_or(0);
                (start, start + r.samples.len())
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nixyon_core::SpeechEdges;

    use crate::config::LONG_PIECE_SAMPLES;
    use crate::long_pieces::split_long;
    use crate::read_wave::read_samples;
    use crate::speech_regions::speech_regions;
    use crate::{ModelManifest, SherpaModels};

    // Папка моделей из MODELS_DIR, относительный путь от корня репозитория.
    fn models_dir() -> PathBuf {
        let dir = PathBuf::from(
            std::env::var("MODELS_DIR").expect("задайте MODELS_DIR: папку с models.json"),
        );
        if dir.is_absolute() {
            dir
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(dir)
        }
    }

    fn plain_bounds(models: &SherpaModels, samples: &[f32]) -> Vec<(usize, usize)> {
        let vad = SherpaModels::new_vad(models.vad_config_for(SpeechEdges::Padded)).unwrap();
        speech_regions(&vad, samples)
            .iter()
            .map(|r| {
                let start = usize::try_from(r.start_sample).unwrap();
                (start, start + r.samples.len())
            })
            .collect()
    }

    #[test]
    #[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
    fn long_vad_pieces_are_cut_by_the_denoised_vad_and_short_ones_stay() {
        let dir = models_dir();
        let manifest = ModelManifest::load(&dir).unwrap();
        assert!(
            manifest.denoiser_path().is_some(),
            "в основном наборе есть шумоподавитель"
        );
        let models = SherpaModels::load(&manifest).unwrap();
        let denoiser = models
            .vad_denoiser()
            .expect("шумоподавитель загружен из набора");
        let samples = read_samples(&dir.join("dialogs/ru6.wav")).unwrap();
        let plain = plain_bounds(&models, &samples);
        let clean = denoiser
            .clean_bounds(models.vad_config_for(SpeechEdges::Padded), &samples)
            .unwrap();
        let cut = split_long(&plain, &clean, LONG_PIECE_SAMPLES);

        let long: Vec<_> = plain
            .iter()
            .filter(|&&(a, b)| b - a >= LONG_PIECE_SAMPLES)
            .collect();
        assert!(!long.is_empty(), "в ru6 есть куски VAD от 15 с");
        for &&(a, b) in &long {
            let inside: Vec<_> = cut.iter().filter(|&&(x, y)| x >= a && y <= b).collect();
            assert!(inside.len() >= 2, "кусок {a}..{b} разрезан: {inside:?}");
        }
        for &(a, b) in plain.iter().filter(|&&(a, b)| b - a < LONG_PIECE_SAMPLES) {
            assert!(cut.contains(&(a, b)), "короткий кусок {a}..{b} остался");
        }
        assert!(cut.windows(2).all(|w| w[0].1 <= w[1].0));
    }

    #[test]
    #[ignore = "нужны модели, см. MODELS_DIR"]
    fn a_recording_without_long_pieces_keeps_its_vad_pieces() {
        let dir = models_dir();
        let models = SherpaModels::load(&ModelManifest::load(&dir).unwrap()).unwrap();
        let samples = read_samples(&dir.join("example.wav")).unwrap();
        let plain = plain_bounds(&models, &samples);
        assert!(plain.iter().all(|&(a, b)| b - a < LONG_PIECE_SAMPLES));
        let with = crate::speech_bounds(&models, &samples, &Default::default()).unwrap();
        let without = crate::speech_bounds(
            &models.without_vad_denoiser(),
            &samples,
            &Default::default(),
        )
        .unwrap();
        assert!(!with.is_empty());
        assert_eq!(with, without);
    }
}
