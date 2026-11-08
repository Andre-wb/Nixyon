//! Хвост записи короче окна шумоподавителя. Нужны модели из scripts/fetch-models.sh:
//! `MODELS_DIR=models cargo test -p nixyon-sherpa -- --ignored`.

use std::path::PathBuf;

use nixyon_core::EngineOptions;
use nixyon_sherpa::{ModelManifest, SherpaModels, read_samples, speech_bounds};

// Звук идёт в шумоподавитель кусками по 0,1 с при 16 кГц.
const CHUNK: usize = 1_600;

fn models_dir() -> PathBuf {
    let dir = std::env::var("MODELS_DIR").expect("задайте MODELS_DIR: папку с models.json");
    let path = PathBuf::from(dir);
    if path.is_absolute() {
        path
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(path)
    }
}

// Последний кусок короче 256 сэмплов: GTCRN на нём отдаёт пустой звук с частотой 0, и
// запись такой длины раньше не разбиралась совсем.
#[test]
#[ignore = "нужны модели, см. MODELS_DIR"]
fn a_short_last_chunk_does_not_stop_the_search_for_speech() {
    let dir = models_dir();
    let manifest = ModelManifest::load(&dir).unwrap();
    assert!(
        manifest.denoiser_path().is_some(),
        "в наборе нет шумоподавителя, тест ничего не проверит"
    );
    let models = SherpaModels::load(&manifest).unwrap();
    let samples = read_samples(&dir.join("example.wav")).unwrap();
    for tail in [1, 192, 255, 256] {
        let len = (samples.len() / CHUNK - 1) * CHUNK + tail;
        let bounds = speech_bounds(&models, &samples[..len], &EngineOptions::default())
            .unwrap_or_else(|e| panic!("хвост {tail} сэмплов: {e}"));
        assert!(!bounds.is_empty(), "хвост {tail} сэмплов: речи нет");
    }
}
