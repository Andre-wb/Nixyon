//! Образцы голоса на настоящих моделях. Нужны модели (scripts/fetch-models.sh) и клипы
//! VoxForge (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-sherpa --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_sherpa::{
    ModelManifest, SherpaError, SherpaModels, sample_embedding, sample_embedding_of,
};

fn models_dir() -> PathBuf {
    let dir = std::env::var("MODELS_DIR").expect("задайте MODELS_DIR: папку с models.json");
    // cargo test запускает тест из папки крейта, относительный путь считаем от корня workspace.
    let path = PathBuf::from(dir);
    if path.is_absolute() {
        path
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(path)
    }
}

fn clip(dir: &Path, name: &str) -> PathBuf {
    dir.join("_download/test-audio/voxforge").join(name)
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
    dot / (norm(a) * norm(b))
}

// WAV 16 кГц, 16 бит, моно из тишины: речи в нём нет.
fn silent_wav(path: &Path, seconds: u32) {
    let samples = 16_000 * seconds;
    let data = samples * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&16_000u32.to_le_bytes());
    bytes.extend_from_slice(&32_000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    bytes.resize(bytes.len() + data as usize, 0);
    std::fs::write(path, bytes).unwrap();
}

#[test]
#[ignore = "нужны модели и клипы, см. MODELS_DIR"]
fn samples_of_one_person_are_closer_than_samples_of_two_people() {
    let dir = models_dir();
    let models = SherpaModels::load(&ModelManifest::load(&dir).unwrap()).unwrap();
    let gav_a = clip(&dir, "gav-20170207-bfb/wav/ru_0043.wav");
    let gav_b = clip(&dir, "gav-20170207-ciw/wav/ru_0003.wav");
    let zend = clip(&dir, "zend-20110502-qab/wav/ru_0049.wav");

    let one = sample_embedding(&models, &gav_a).unwrap();
    let same = sample_embedding(&models, &gav_b).unwrap();
    let other = sample_embedding(&models, &zend).unwrap();
    let both = sample_embedding_of(&models, &[gav_a.as_path(), gav_b.as_path()]).unwrap();

    assert_eq!(one.len(), models.embedding_dim() as usize);
    assert!(
        cosine(&one, &same) > cosine(&one, &other),
        "свой {:.3}, чужой {:.3}",
        cosine(&one, &same),
        cosine(&one, &other)
    );
    assert!(cosine(&both, &one) > cosine(&both, &other));
}

#[test]
#[ignore = "нужны модели, см. MODELS_DIR"]
fn sample_without_speech_is_refused() {
    let dir = models_dir();
    let models = SherpaModels::load(&ModelManifest::load(&dir).unwrap()).unwrap();
    let wav = std::env::temp_dir().join(format!("nixyon-silent-{}.wav", std::process::id()));
    silent_wav(&wav, 5);
    let result = sample_embedding(&models, &wav);
    std::fs::remove_file(&wav).unwrap();
    assert!(matches!(result, Err(SherpaError::SampleTooShort(_))));
}
