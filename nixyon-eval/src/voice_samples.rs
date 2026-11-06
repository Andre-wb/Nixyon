use std::path::{Path, PathBuf};

use nixyon_audio::convert_to_wav16k;
use nixyon_core::VoiceSample;
use nixyon_sherpa::{SherpaModels, sample_embedding_of};
use serde::Deserialize;

/// Файл образцов для замера (`scripts/data/voice-samples.json`): у каждого голоса
/// имя и клипы, пути относительно папки скачанных записей.
#[derive(Debug, Deserialize)]
struct SamplesFile {
    samples: Vec<SampleEntry>,
}

#[derive(Debug, Deserialize)]
struct SampleEntry {
    name: String,
    clips: Vec<PathBuf>,
}

/// Образцы голоса из файла: клипы переводятся в 16 кГц тем же путём, что в приложении,
/// эмбеддинг считается по речи всех клипов голоса подряд.
pub fn load_voice_samples(
    models: &SherpaModels,
    file: &Path,
    audio_root: &Path,
) -> Result<Vec<VoiceSample>, Box<dyn std::error::Error>> {
    let parsed: SamplesFile = serde_json::from_str(&std::fs::read_to_string(file)?)?;
    let mut samples = Vec::with_capacity(parsed.samples.len());
    for entry in parsed.samples {
        let mut converted: Vec<PathBuf> = Vec::new();
        let result = (|| -> Result<Vec<f32>, Box<dyn std::error::Error>> {
            for (i, clip) in entry.clips.iter().enumerate() {
                let out = std::env::temp_dir().join(format!(
                    "nixyon-sample-{}-{}-{i}.wav",
                    std::process::id(),
                    entry.name
                ));
                converted.push(out.clone());
                convert_to_wav16k(&audio_root.join(clip), &out)?;
            }
            let paths: Vec<&Path> = converted.iter().map(PathBuf::as_path).collect();
            Ok(sample_embedding_of(models, &paths)?)
        })();
        // Временные файлы удаляются при любом исходе; ошибка удаления не важнее ошибки разбора.
        for path in &converted {
            let _ = std::fs::remove_file(path);
        }
        samples.push(VoiceSample {
            name: entry.name,
            embedding: result?,
        });
    }
    Ok(samples)
}
