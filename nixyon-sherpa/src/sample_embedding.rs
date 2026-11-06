use std::path::Path;

use nixyon_core::MIN_SAMPLE_SPEECH_SEC;

use crate::analyze::embed;
use crate::config::SAMPLE_RATE;
use crate::read_wave::read_wave;
use crate::speech_regions::speech_regions;
use crate::{SherpaError, SherpaModels};

/// Эмбеддинг голоса по образцу: речь, которую нашёл VAD, склеивается подряд, тишина
/// и шум между фразами отбрасываются. Речи меньше `MIN_SAMPLE_SPEECH_SEC` секунд
/// или эмбеддинга нет: ошибка `SampleTooShort`.
pub fn sample_embedding(models: &SherpaModels, wav: &Path) -> Result<Vec<f32>, SherpaError> {
    sample_embedding_of(models, &[wav])
}

/// То же по нескольким WAV 16 кГц одного человека: речь из всех файлов идёт подряд.
/// Нужно замеру, где образец собирается из нескольких клипов.
pub fn sample_embedding_of(models: &SherpaModels, wavs: &[&Path]) -> Result<Vec<f32>, SherpaError> {
    let mut speech: Vec<f32> = Vec::new();
    for wav in wavs {
        let wave = read_wave(wav)?;
        let vad = SherpaModels::new_vad(models.vad_config())?;
        speech.extend(
            speech_regions(&vad, wave.samples())
                .into_iter()
                .flat_map(|r| r.samples),
        );
    }
    let speech_sec = speech.len() as f64 / f64::from(SAMPLE_RATE);
    if speech_sec < MIN_SAMPLE_SPEECH_SEC {
        return Err(SherpaError::SampleTooShort(speech_sec));
    }
    let embedding = embed(models, &speech)?;
    if embedding.is_empty() {
        return Err(SherpaError::SampleTooShort(speech_sec));
    }
    Ok(embedding)
}
