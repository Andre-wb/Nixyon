//! Нейросегментация на настоящих моделях. Нужны модели (scripts/fetch-models.sh)
//! и диалоги (scripts/fetch-test-audio.sh).
//! `MODELS_DIR=models cargo test -p nixyon-sherpa --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_core::{CancelFlag, EngineOptions, NEURAL_BLOCK_SEC};
use nixyon_sherpa::{ModelManifest, SherpaError, SherpaModels, analyze, voice_spans};

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

fn load(dir: &Path) -> SherpaModels {
    SherpaModels::load(&ModelManifest::load(dir).unwrap()).unwrap()
}

// Набор без модели сегментации: ссылки на файлы основного набора и манифест без этой роли.
fn set_without_segmentation(dir: &Path) -> PathBuf {
    let set = std::env::temp_dir().join(format!("nixyon-no-seg-{}", std::process::id()));
    std::fs::create_dir_all(&set).unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("models.json")).unwrap()).unwrap();
    let files = manifest["files"].as_object_mut().unwrap();
    files.remove("segmentation");
    for file in files.values() {
        let name = file["path"].as_str().unwrap();
        let link = set.join(name);
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(dir.join(name), &link).unwrap();
    }
    std::fs::write(set.join("models.json"), manifest.to_string()).unwrap();
    set
}

#[test]
#[ignore = "нужны модели, см. MODELS_DIR"]
fn overlapping_voices_give_spans_that_overlap_in_time_inside_speech() {
    let dir = models_dir();
    let models = load(&dir);
    let wav = dir.join("dialogs/ru4_overlap.wav");
    let segments = analyze(&models, &wav, &EngineOptions::default(), &CancelFlag::new()).unwrap();
    let speech: Vec<(f64, f64)> = segments.iter().map(|s| (s.start, s.end)).collect();
    let spans = voice_spans(&models, &wav, &speech, NEURAL_BLOCK_SEC, &CancelFlag::new()).unwrap();

    assert!(spans.len() > 10, "отрезков {}", spans.len());
    for s in &spans {
        assert!(s.start < s.end);
        assert!(!s.embedding.is_empty());
        assert!(
            speech.iter().any(|&(a, b)| a <= s.start && s.end <= b),
            "отрезок {}..{} вне кусков VAD",
            s.start,
            s.end
        );
    }
    assert!(spans.windows(2).all(|w| w[0].start <= w[1].start));
    // В записи 11 наложений голосов по 0,5..1,5 с.
    let overlapping = spans
        .iter()
        .enumerate()
        .flat_map(|(i, a)| spans[i + 1..].iter().map(move |b| (a, b)))
        .filter(|(a, b)| a.end.min(b.end) - a.start.max(b.start) > 0.2)
        .count();
    assert!(overlapping >= 3, "пересечений {overlapping}");
}

#[test]
#[ignore = "нужны модели, см. MODELS_DIR"]
fn set_without_segmentation_model_refuses_neural_segmentation() {
    let dir = models_dir();
    let set = set_without_segmentation(&dir);
    let models = load(&set);
    let err = voice_spans(
        &models,
        &dir.join("example.wav"),
        &[(0.0, 1.0)],
        NEURAL_BLOCK_SEC,
        &CancelFlag::new(),
    )
    .unwrap_err();
    std::fs::remove_dir_all(&set).unwrap();
    assert!(matches!(err, SherpaError::NoSegmentationModel));
}

#[test]
#[ignore = "нужны модели, см. MODELS_DIR"]
fn cancelled_segmentation_stops() {
    let dir = models_dir();
    let models = load(&dir);
    let cancel = CancelFlag::new();
    cancel.cancel();
    let err = voice_spans(
        &models,
        &dir.join("dialogs/ru2.wav"),
        &[(0.0, 60.0)],
        NEURAL_BLOCK_SEC,
        &cancel,
    )
    .unwrap_err();
    assert!(matches!(err, SherpaError::Cancelled));
}
