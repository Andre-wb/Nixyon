//! Перекрытия по модели сегментации pyannote на настоящих записях. Запуск:
//! `MODELS_DIR=models cargo test -p nixyon-ort --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_core::{ModelOverlaps, POWERSET_CLASSES};
use nixyon_ort::{PyannotePowerset, scan_overlaps};

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

fn scan(dir: &Path, record: &str) -> ModelOverlaps {
    let model = PyannotePowerset::load(&dir.join("segmentation.onnx"), 2).unwrap();
    let wav = dir.join(format!("dialogs/{record}.wav"));
    scan_overlaps(&model, &wav, 0.3, 0.3, &mut |_| true)
        .unwrap()
        .unwrap()
}

#[test]
#[ignore = "нужны модели и диалоги: MODELS_DIR"]
fn record_without_overlaps_gives_no_spans_and_keeps_no_windows() {
    let found = scan(&models_dir(), "ru2");
    assert!(found.spans.is_empty(), "{:?}", found.spans);
    assert!(found.windows.is_empty());
}

#[test]
#[ignore = "нужны модели и диалоги: MODELS_DIR"]
fn mixed_voices_are_found_and_only_windows_around_them_are_kept() {
    // В ru4_overlap наложение у смены говорящих 71,65..72,15 с.
    let found = scan(&models_dir(), "ru4_overlap");
    assert!(
        found.spans.iter().any(|&(a, b)| a < 72.15 && 71.65 < b),
        "{:?}",
        found.spans
    );
    for w in &found.windows {
        let from = w.frame_time(0);
        let to = w.frame_time(w.frames.len() - 1);
        assert!(
            found
                .spans
                .iter()
                .any(|&(a, b)| a < to + 0.1 && from - 0.1 < b)
        );
        for f in &w.frames {
            assert_eq!(f.len(), POWERSET_CLASSES);
            assert!((f.iter().sum::<f32>() - 1.0).abs() < 1e-4);
        }
    }
    let audio_windows = 87;
    assert!(
        found.windows.len() < audio_windows,
        "{}",
        found.windows.len()
    );
}

#[test]
#[ignore = "нужны модели и диалоги: MODELS_DIR"]
fn cancelled_scan_gives_nothing() {
    let dir = models_dir();
    let model = PyannotePowerset::load(&dir.join("segmentation.onnx"), 2).unwrap();
    let wav = dir.join("dialogs/ru4_overlap.wav");
    assert_eq!(
        scan_overlaps(&model, &wav, 0.3, 0.3, &mut |_| false).unwrap(),
        None
    );
}
