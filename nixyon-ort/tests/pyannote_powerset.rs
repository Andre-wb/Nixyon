//! Выход нейросегментации pyannote по кадрам на настоящих записях. Запуск:
//! `MODELS_DIR=models cargo test -p nixyon-ort --release -- --ignored`.

use std::path::PathBuf;

use nixyon_core::{CancelFlag, PowersetWindow, overlap_probability};
use nixyon_ort::PyannotePowerset;

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

fn windows(dir: &std::path::Path, record: &str) -> (Vec<PowersetWindow>, f64) {
    let model = PyannotePowerset::load(&dir.join("segmentation.onnx"), 2).unwrap();
    let samples = nixyon_sherpa::read_samples(&dir.join(format!("dialogs/{record}.wav"))).unwrap();
    let duration = samples.len() as f64 / 16_000.0;
    let windows = model
        .windows(&samples, &CancelFlag::new())
        .unwrap()
        .unwrap();
    (windows, duration)
}

#[test]
#[ignore = "нужны модели и диалоги: MODELS_DIR"]
fn windows_cover_the_record_and_frames_are_probabilities() {
    let (windows, duration) = windows(&models_dir(), "ru2");
    assert!(!windows.is_empty());
    let last = windows.last().unwrap();
    let end = last.frame_time(last.frames.len() - 1);
    assert!(
        end < duration && duration - end < 0.1,
        "{end} из {duration}"
    );
    for w in &windows {
        for f in &w.frames {
            assert!((f.iter().sum::<f32>() - 1.0).abs() < 1e-4);
        }
    }
}

#[test]
#[ignore = "нужны модели и диалоги: MODELS_DIR"]
fn overlap_is_more_likely_where_two_voices_were_mixed() {
    // В ru4_overlap наложения у смены говорящих, например 71,65..72,15 с; в 2..5 с говорит
    // один человек.
    let (windows, _) = windows(&models_dir(), "ru4_overlap");
    let mean = |from: f64, to: f64| {
        let values: Vec<f32> = windows
            .iter()
            .flat_map(|w| {
                (0..w.frames.len())
                    .filter(move |&i| (from..to).contains(&w.frame_time(i)))
                    .map(move |i| overlap_probability(&w.frames[i]))
            })
            .collect();
        values.iter().sum::<f32>() / values.len() as f32
    };
    let mixed = mean(71.65, 72.15);
    let single = mean(2.0, 5.0);
    assert!(mixed > 0.3 && single < 0.05, "{mixed} и {single}");
}
