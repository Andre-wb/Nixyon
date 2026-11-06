use sherpa_onnx::{AudioTagging, AudioTaggingConfig, AudioTaggingModelConfig};

use crate::config::SAMPLE_RATE;
use crate::{ModelManifest, SherpaError};

// Классы AudioSet в списке модели: речь и её виды, музыка и пение.
const SPEECH_CLASSES: [i32; 6] = [0, 1, 2, 3, 4, 5];
const MUSIC_CLASSES: [i32; 6] = [137, 27, 32, 33, 34, 35];
/// Сколько классов просить у модели: все, иначе нужные могут не попасть в ответ.
const ALL_CLASSES: i32 = 527;
/// Музыка должна быть уверенной: вероятности классов AudioSet независимы.
const MUSIC_MIN_PROB: f32 = 0.5;

/// Отличает куски, где вместо речи звучит музыка или пение: модель распознавания
/// звуков CED по классам AudioSet.
pub struct MusicCheck {
    tagging: AudioTagging,
}

impl MusicCheck {
    /// None, если в наборе моделей нет распознавания звуков.
    pub fn load(manifest: &ModelManifest) -> Result<Option<Self>, SherpaError> {
        let Some((model, labels)) = manifest.tagging_paths() else {
            return Ok(None);
        };
        let config = AudioTaggingConfig {
            model: AudioTaggingModelConfig {
                ced: Some(model),
                num_threads: 1,
                ..AudioTaggingModelConfig::default()
            },
            labels: Some(labels),
            top_k: ALL_CLASSES,
        };
        let tagging = AudioTagging::create(&config)
            .ok_or(SherpaError::CreateFailed("распознавание звуков"))?;
        Ok(Some(Self { tagging }))
    }

    /// Звучит ли в куске (16 кГц) музыка вместо речи.
    pub fn is_music(&self, samples: &[f32]) -> bool {
        let stream = self.tagging.create_stream();
        stream.accept_waveform(SAMPLE_RATE, samples);
        let events: Vec<(i32, f32)> = self
            .tagging
            .compute(&stream, ALL_CLASSES)
            .into_iter()
            .map(|e| (e.index, e.prob))
            .collect();
        music_wins(&events)
    }
}

/// Музыка или пение уверенны и сильнее речи. Ведущий поверх фоновой музыки остаётся:
/// у него речь сильнее.
pub(crate) fn music_wins(events: &[(i32, f32)]) -> bool {
    let strongest = |classes: &[i32]| {
        events
            .iter()
            .filter(|(index, _)| classes.contains(index))
            .map(|&(_, prob)| prob)
            .fold(0.0f32, f32::max)
    };
    let music = strongest(&MUSIC_CLASSES);
    music >= MUSIC_MIN_PROB && music > strongest(&SPEECH_CLASSES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_is_music_and_a_host_over_music_is_speech() {
        assert!(music_wins(&[(137, 0.9), (27, 0.7), (0, 0.3)]), "песня");
        assert!(music_wins(&[(33, 0.8), (0, 0.4)]), "пение");
        assert!(
            !music_wins(&[(137, 0.7), (0, 0.9)]),
            "ведущий на фоне музыки"
        );
        assert!(!music_wins(&[(137, 0.4), (0, 0.1)]), "музыка неуверенная");
        assert!(!music_wins(&[(0, 0.95)]), "речь");
        assert!(!music_wins(&[]));
    }
}
