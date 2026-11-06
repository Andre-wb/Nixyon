//! Горячие слова на настоящих моделях. Нужны модели со словарём BPE
//! (scripts/fetch-models.sh) и клипы VoxForge (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-sherpa --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_core::{CancelFlag, EngineOptions};
use nixyon_sherpa::{Decoding, ModelManifest, SherpaError, SherpaModels, analyze};

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

// Клип, где без подсказки имя распознаётся как «Арчи Бальт».
fn clip(dir: &Path) -> PathBuf {
    dir.join("_download/test-audio/voxforge/polazhenko-20140825-cvh/wav/ru_0001.wav")
}

fn text(models: &SherpaModels, wav: &Path, hotwords: &[&str]) -> Result<String, SherpaError> {
    let options = EngineOptions {
        hotwords: hotwords.iter().map(|h| h.to_string()).collect(),
        ..EngineOptions::default()
    };
    let segments = analyze(models, wav, &options, &CancelFlag::new())?;
    Ok(segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" "))
}

#[test]
#[ignore = "нужны модели и клипы, см. MODELS_DIR"]
fn hotword_fixes_a_name_the_model_splits() {
    let dir = models_dir();
    let manifest = ModelManifest::load(&dir).unwrap();
    let beam = SherpaModels::load_with(&manifest, Decoding::default()).unwrap();
    let wav = clip(&dir);

    let plain = text(&beam, &wav, &[]).unwrap();
    assert!(!plain.contains("Арчибальд"), "{plain}");
    let hinted = text(&beam, &wav, &["Арчибальд"]).unwrap();
    assert!(hinted.contains("Арчибальд"), "{hinted}");
}

#[test]
#[ignore = "нужны модели и клипы, см. MODELS_DIR"]
fn hotwords_with_greedy_search_are_refused() {
    let dir = models_dir();
    let greedy =
        SherpaModels::load_with(&ModelManifest::load(&dir).unwrap(), Decoding::Greedy).unwrap();
    assert!(matches!(
        text(&greedy, &clip(&dir), &["Арчибальд"]),
        Err(SherpaError::HotwordsNeedBeam)
    ));
}
