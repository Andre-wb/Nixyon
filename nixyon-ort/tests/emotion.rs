//! Эмоция по голосу на настоящей модели GigaAM-Emo. Нужны основной набор моделей
//! (scripts/fetch-models.sh) и клипы VoxForge (scripts/fetch-test-audio.sh): в VoxForge
//! люди спокойно читают текст вслух, это нейтральная речь.
//! `MODELS_DIR=models cargo test -p nixyon-ort --release --test emotion -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_core::TurnEmotion;
use nixyon_ort::{EmotionModel, EmotionProbs, OrtManifest, OrtRole};

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

fn model_path(dir: &Path) -> PathBuf {
    OrtManifest::load(dir)
        .unwrap()
        .path(OrtRole::Emotion)
        .unwrap()
}

fn model(dir: &Path) -> EmotionModel {
    EmotionModel::load(&model_path(dir), 2).unwrap()
}

fn samples(path: &Path) -> Vec<f32> {
    let wave = sherpa_onnx::Wave::read(path.to_str().unwrap()).unwrap();
    assert_eq!(wave.sample_rate(), 16_000);
    wave.samples().to_vec()
}

const CALM_CLIPS: [&str; 3] = [
    "gav-20170207-bfb/wav/ru_0043.wav",
    "zend-20110502-qab/wav/ru_0049.wav",
    "kalan-20100212-jnx/wav/ru_0003.wav",
];

fn calm(dir: &Path, clip: &str) -> Vec<f32> {
    samples(&dir.join("_download/test-audio/voxforge").join(clip))
}

fn assert_probabilities(p: &EmotionProbs) {
    let sum = p.angry + p.sad + p.neutral + p.positive;
    assert!((sum - 1.0).abs() < 1e-4, "{p:?}");
}

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn calm_reading_is_recognized_as_neutral() {
    let dir = models_dir();
    let m = model(&dir);
    let mut clips: Vec<Vec<f32>> = CALM_CLIPS.iter().map(|c| calm(&dir, c)).collect();
    clips.push(samples(&dir.join("example.wav")));
    for clip in clips {
        let p = m.probs(&clip).unwrap().unwrap();
        assert_probabilities(&p);
        assert_eq!(
            p.emotion(EmotionProbs::THRESHOLD),
            Some(TurnEmotion::Neutral),
            "{p:?}"
        );
    }
}

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn a_piece_longer_than_the_window_is_averaged_over_windows() {
    let dir = models_dir();
    let m = model(&dir);
    let clip = calm(&dir, CALM_CLIPS[0]);
    let mut long = Vec::new();
    while long.len() < 20 * 16_000 {
        long.extend_from_slice(&clip);
    }
    let p = m.probs(&long).unwrap().unwrap();
    assert_probabilities(&p);
    assert_eq!(
        p.emotion(EmotionProbs::THRESHOLD),
        Some(TurnEmotion::Neutral),
        "{p:?}"
    );
}

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn a_tiny_piece_gets_no_estimate_and_a_short_one_does() {
    let dir = models_dir();
    let m = model(&dir);
    assert_eq!(m.probs(&[0.0; 3_999]).unwrap(), None);
    let clip = calm(&dir, CALM_CLIPS[1]);
    let p = m.probs(&clip[..4_000]).unwrap().unwrap();
    assert_probabilities(&p);
}

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn the_model_loaded_from_bytes_gives_the_probabilities_of_the_file() {
    let dir = models_dir();
    let bytes = std::fs::read(model_path(&dir)).unwrap();
    let from_bytes = EmotionModel::from_bytes(&bytes, 2).unwrap();
    let clip = calm(&dir, CALM_CLIPS[2]);
    assert_eq!(
        from_bytes.probs(&clip).unwrap(),
        model(&dir).probs(&clip).unwrap()
    );
}
