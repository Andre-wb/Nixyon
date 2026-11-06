//! Пол и возраст по голосу на настоящей модели audEERING. Нужны набор ort
//! (scripts/fetch-models.sh) и клипы VoxForge (scripts/fetch-test-audio.sh), у всех
//! говорящих VoxForge в разметке указаны мужчины:
//! `MODELS_DIR=models cargo test -p nixyon-ort --release --test age_gender -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_ort::{AgeGender, OrtManifest, OrtRole, VoiceTraits};

const WINDOW_SEC: f64 = 10.0;

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

fn model(dir: &Path, sessions: usize) -> AgeGender {
    let path = OrtManifest::load(&dir.join("ort"))
        .unwrap()
        .path(OrtRole::AgeGender)
        .unwrap();
    AgeGender::load_pool(&path, 2, sessions, WINDOW_SEC).unwrap()
}

fn samples(dir: &Path, clip: &str) -> Vec<f32> {
    let path = dir.join("_download/test-audio/voxforge").join(clip);
    let wave = sherpa_onnx::Wave::read(path.to_str().unwrap()).unwrap();
    assert_eq!(wave.sample_rate(), 16_000);
    wave.samples().to_vec()
}

fn assert_adult_male(t: &VoiceTraits) {
    assert!((t.female + t.male + t.child - 1.0).abs() < 1e-4, "{t:?}");
    assert!(t.male > t.female && t.male > t.child, "{t:?}");
    assert!(t.age_years > 13.0, "{t:?}");
}

const MALE_CLIPS: [&str; 3] = [
    "gav-20170207-bfb/wav/ru_0043.wav",
    "zend-20110502-qab/wav/ru_0049.wav",
    "kalan-20100212-jnx/wav/ru_0003.wav",
];

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn clips_of_men_are_recognized_as_adult_male_voices() {
    let dir = models_dir();
    let m = model(&dir, 1);
    for clip in MALE_CLIPS {
        let t = m.traits(&samples(&dir, clip)).unwrap().unwrap();
        assert_adult_male(&t);
    }
}

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn a_piece_longer_than_the_window_is_averaged_over_windows() {
    let dir = models_dir();
    let m = model(&dir, 1);
    let mut long = Vec::new();
    while long.len() < (WINDOW_SEC * 2.5 * 16_000.0) as usize {
        long.extend(samples(&dir, MALE_CLIPS[0]));
    }
    let t = m.traits(&long).unwrap().unwrap();
    assert_adult_male(&t);
}

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn a_tiny_piece_gets_no_estimate() {
    let dir = models_dir();
    assert_eq!(model(&dir, 1).traits(&[0.0; 399]).unwrap(), None);
}

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn pooled_sessions_in_parallel_give_the_estimates_of_a_single_session() {
    let dir = models_dir();
    let single = model(&dir, 1);
    let pooled = model(&dir, 3);
    let clips: Vec<Vec<f32>> = MALE_CLIPS.iter().map(|c| samples(&dir, c)).collect();
    let expected: Vec<VoiceTraits> = clips
        .iter()
        .map(|s| single.traits(s).unwrap().unwrap())
        .collect();
    let got: Vec<VoiceTraits> = std::thread::scope(|scope| {
        let handles: Vec<_> = clips
            .iter()
            .map(|s| scope.spawn(|| pooled.traits(s).unwrap().unwrap()))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (a, b) in expected.iter().zip(&got) {
        assert!((a.male - b.male).abs() < 1e-5 && (a.age_years - b.age_years).abs() < 1e-3);
    }
}

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn the_analysis_gives_the_same_traits_and_hidden_states_of_the_model_length() {
    let dir = models_dir();
    let m = model(&dir, 1);
    let s = samples(&dir, MALE_CLIPS[1]);
    let a = m.analyze(&s).unwrap().unwrap();
    assert_eq!(Some(a.traits), m.traits(&s).unwrap());
    assert_eq!(a.hidden_states.len(), 1024);
    assert!(a.hidden_states.iter().all(|x| x.is_finite()));
}

#[test]
#[ignore = "нужны модели: MODELS_DIR"]
fn the_model_loaded_from_bytes_gives_the_analysis_of_the_file() {
    let dir = models_dir();
    let path = OrtManifest::load(&dir.join("ort"))
        .unwrap()
        .path(OrtRole::AgeGender)
        .unwrap();
    let from_file = model(&dir, 1);
    let from_bytes = AgeGender::from_bytes(&std::fs::read(&path).unwrap(), 2, WINDOW_SEC).unwrap();
    let clip = samples(&dir, MALE_CLIPS[0]);
    let a = from_file.analyze(&clip).unwrap().unwrap();
    let b = from_bytes.analyze(&clip).unwrap().unwrap();
    assert_eq!(a.hidden_states.len(), b.hidden_states.len());
    let diff = a
        .hidden_states
        .iter()
        .zip(&b.hidden_states)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max);
    assert!(diff < 1e-4, "наибольшая разница {diff}");
    assert!((a.traits.child - b.traits.child).abs() < 1e-4);
}
