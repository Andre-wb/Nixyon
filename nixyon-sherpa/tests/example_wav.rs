//! Прогон настоящих моделей. Нужны файлы из scripts/fetch-models.sh:
//! `MODELS_DIR=models cargo test -p nixyon-sherpa -- --ignored`.

use std::path::PathBuf;

use nixyon_core::{CancelFlag, EngineOptions, WindowSpec};
use nixyon_sherpa::{ModelManifest, SherpaModels, analyze};

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

#[test]
#[ignore = "нужны модели, см. MODELS_DIR"]
fn example_wav_gives_text_token_times_and_voice_embeddings() {
    let dir = models_dir();
    let manifest = ModelManifest::load(&dir).unwrap();
    let models = SherpaModels::load(&manifest).unwrap();
    let segments = analyze(
        &models,
        &dir.join("example.wav"),
        &EngineOptions::default(),
        &CancelFlag::new(),
    )
    .unwrap();

    assert!(!segments.is_empty());
    for segment in &segments {
        assert!(segment.start < segment.end);
        assert!(!segment.text.is_empty());
        assert_eq!(segment.tokens.len(), segment.token_times.len());
        assert_eq!(segment.embedding.len(), 192);
    }
    assert!(segments.windows(2).all(|w| w[0].end <= w[1].start));
    assert!(segments.iter().all(|s| s.windows.is_empty()));
}

#[test]
#[ignore = "нужны модели, см. MODELS_DIR"]
fn requested_windows_cover_each_long_segment_to_its_end() {
    let dir = models_dir();
    let models = SherpaModels::load(&ModelManifest::load(&dir).unwrap()).unwrap();
    let spec = WindowSpec::new(1.5, 0.5).unwrap();
    let options = EngineOptions {
        windows: Some(spec),
        ..EngineOptions::default()
    };
    let segments = analyze(
        &models,
        &dir.join("example.wav"),
        &options,
        &CancelFlag::new(),
    )
    .unwrap();

    let long: Vec<_> = segments.iter().filter(|s| s.end - s.start >= 1.5).collect();
    assert!(!long.is_empty());
    for segment in long {
        let w = &segment.windows;
        assert!(!w.is_empty());
        assert!(w.iter().all(|x| x.embedding.len() == 192));
        assert!(
            w.iter()
                .all(|x| x.start >= segment.start && x.end <= segment.end + 1e-9)
        );
        assert!(w.iter().all(|x| (x.end - x.start - 1.5).abs() < 1e-3));
        assert!((w.last().unwrap().end - segment.end).abs() < 1e-3);
    }
}

#[test]
#[ignore = "нужны модели, см. MODELS_DIR"]
fn wav_with_wrong_sample_rate_is_rejected() {
    let dir = models_dir();
    let models = SherpaModels::load(&ModelManifest::load(&dir).unwrap()).unwrap();
    let wav = std::env::temp_dir().join(format!("nixyon-8k-{}.wav", std::process::id()));
    assert!(sherpa_onnx::write(
        &wav.to_string_lossy(),
        &[0.0; 8000],
        8000
    ));
    let err = analyze(&models, &wav, &EngineOptions::default(), &CancelFlag::new()).unwrap_err();
    std::fs::remove_file(&wav).unwrap();
    assert!(err.to_string().contains("16000"));
}
