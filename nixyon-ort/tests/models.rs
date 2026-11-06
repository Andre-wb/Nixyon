//! ReDimNet2 и Sortformer на настоящих моделях вместе с моделями sherpa-onnx. Нужны
//! основной набор и набор ort (scripts/fetch-models.sh), клипы и диалоги
//! (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-ort --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_core::{CancelFlag, EngineOptions, NEURAL_BLOCK_SEC, VoiceEmbedder, VoiceSpan};
use nixyon_ort::{Binarize, OrtManifest, OrtRole, Redimnet2, Sortformer};
use nixyon_sherpa::{
    ModelManifest, SherpaModels, analyze, sample_embedding, sample_embedding_of, voice_spans,
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

fn ort_path(dir: &Path, role: OrtRole) -> PathBuf {
    OrtManifest::load(&dir.join("ort"))
        .unwrap()
        .path(role)
        .unwrap()
}

fn base_models(dir: &Path) -> SherpaModels {
    SherpaModels::load(&ModelManifest::load(dir).unwrap()).unwrap()
}

fn redimnet(dir: &Path) -> Redimnet2 {
    Redimnet2::load(&ort_path(dir, OrtRole::Redimnet2), 4).unwrap()
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

fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn dialog_samples(dir: &Path, name: &str) -> Vec<f32> {
    let path = dir.join(format!("dialogs/{name}.wav"));
    sherpa_onnx::Wave::read(path.to_str().unwrap())
        .unwrap()
        .samples()
        .to_vec()
}

fn sortformer_spans(dir: &Path, name: &str) -> (Vec<(f64, f64)>, Vec<VoiceSpan>) {
    let sortformer = Sortformer::load(
        &ort_path(dir, OrtRole::SortformerInt8),
        4,
        Binarize::default(),
    )
    .unwrap();
    let models = base_models(dir).with_segmenter(Box::new(sortformer));
    let wav = dir.join(format!("dialogs/{name}.wav"));
    let segments = analyze(&models, &wav, &EngineOptions::default(), &CancelFlag::new()).unwrap();
    let speech: Vec<(f64, f64)> = segments.iter().map(|s| (s.start, s.end)).collect();
    let spans = voice_spans(&models, &wav, &speech, NEURAL_BLOCK_SEC, &CancelFlag::new()).unwrap();
    (speech, spans)
}

#[test]
#[ignore = "нужны модели и клипы, см. MODELS_DIR"]
fn redimnet_samples_of_one_person_are_closer_than_samples_of_two_people() {
    let dir = models_dir();
    let models = base_models(&dir).with_voice(Box::new(redimnet(&dir)));
    let gav_a = clip(&dir, "gav-20170207-bfb/wav/ru_0043.wav");
    let gav_b = clip(&dir, "gav-20170207-ciw/wav/ru_0003.wav");
    let zend = clip(&dir, "zend-20110502-qab/wav/ru_0049.wav");

    let one = sample_embedding(&models, &gav_a).unwrap();
    let same = sample_embedding(&models, &gav_b).unwrap();
    let other = sample_embedding(&models, &zend).unwrap();
    let both = sample_embedding_of(&models, &[gav_a.as_path(), gav_b.as_path()]).unwrap();

    assert_eq!(one.len(), 192);
    assert_eq!(models.embedding_dim(), 192);
    assert!(
        cosine(&one, &same) > cosine(&one, &other),
        "свой {:.3}, чужой {:.3}",
        cosine(&one, &same),
        cosine(&one, &other)
    );
    assert!(cosine(&both, &one) > cosine(&both, &other));
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn redimnet_gives_unit_embeddings_for_short_and_long_pieces_and_none_for_a_tiny_one() {
    let dir = models_dir();
    let model = redimnet(&dir);
    let samples = dialog_samples(&dir, "ru2");

    let short = model.embed(&samples[16_000..40_000]).unwrap();
    let long = model.embed(&samples[..16_000 * 20]).unwrap();
    assert_eq!(short.len(), model.dim());
    assert_eq!(long.len(), model.dim());
    assert!((norm(&short) - 1.0).abs() < 1e-3, "норма {}", norm(&short));
    assert!((norm(&long) - 1.0).abs() < 1e-3, "норма {}", norm(&long));
    assert!(model.embed(&samples[..399]).unwrap().is_empty());
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn redimnet_loaded_from_bytes_gives_the_same_embeddings_as_from_the_file() {
    let dir = models_dir();
    let path = ort_path(&dir, OrtRole::Redimnet2);
    let from_file = Redimnet2::load(&path, 4).unwrap();
    let from_bytes = Redimnet2::from_bytes(&std::fs::read(&path).unwrap(), 4).unwrap();
    let samples = dialog_samples(&dir, "ru2");

    assert_eq!(from_bytes.dim(), from_file.dim());
    for piece in [&samples[16_000..40_000], &samples[..16_000 * 20]] {
        let a = from_file.embed(piece).unwrap();
        let b = from_bytes.embed(piece).unwrap();
        assert_eq!(a.len(), b.len());
        assert!(cosine(&a, &b) > 0.999_999, "косинус {}", cosine(&a, &b));
    }
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn sortformer_spans_follow_the_turns_of_a_calm_dialog() {
    let dir = models_dir();
    let (speech, spans) = sortformer_spans(&dir, "ru2");
    assert!(!spans.is_empty());
    for s in &spans {
        assert!(s.start < s.end);
        assert_eq!(s.embedding.len(), 192);
        assert!(
            speech.iter().any(|&(a, b)| a <= s.start && s.end <= b),
            "отрезок {}..{} вне кусков VAD",
            s.start,
            s.end
        );
    }
    assert!(spans.windows(2).all(|w| w[0].start <= w[1].start));
    // Первая реплика по разметке 0,69..6,62 с, вторая 8,305..11,665 с.
    let covers = |a: f64, b: f64| spans.iter().any(|s| s.start <= a && b <= s.end);
    assert!(covers(1.0, 6.3), "первая реплика не покрыта");
    assert!(covers(8.6, 11.4), "вторая реплика не покрыта");
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn sortformer_marks_overlapping_voices() {
    let dir = models_dir();
    let (_, spans) = sortformer_spans(&dir, "ru4_overlap");
    let overlapping = spans
        .iter()
        .enumerate()
        .flat_map(|(i, a)| spans[i + 1..].iter().map(move |b| (a, b)))
        .filter(|(a, b)| a.end.min(b.end) - a.start.max(b.start) > 0.2)
        .count();
    assert!(overlapping >= 2, "пересечений {overlapping}");
}

// Свои выгрузки ReDimNet2 с динамическим входом.
const DYNAMIC_ROLES: [OrtRole; 6] = [
    OrtRole::Redimnet2B0,
    OrtRole::Redimnet2B1,
    OrtRole::Redimnet2B2,
    OrtRole::Redimnet2B3,
    OrtRole::Redimnet2B3Vb2,
    OrtRole::Redimnet2B6Dyn,
];

// Реплики живых диалогов по голосу из разметки: голос и звук от начала реплики.
fn turns_by_voice(dir: &Path, names: &[&str]) -> Vec<(String, Vec<f32>)> {
    let mut turns = Vec::new();
    for name in names {
        let samples = dialog_samples(dir, name);
        let text = std::fs::read_to_string(dir.join(format!("dialogs/{name}.truth.json"))).unwrap();
        let truth: serde_json::Value = serde_json::from_str(&text).unwrap();
        for turn in truth["turns"].as_array().unwrap() {
            let at = |key: &str| (turn[key].as_f64().unwrap() * 16_000.0) as usize;
            let voice = turn["voice"].as_str().unwrap().to_owned();
            turns.push((
                voice,
                samples[at("start")..at("end").min(samples.len())].to_vec(),
            ));
        }
    }
    turns
}

// Средний косинус пар одного голоса минус средний косинус пар разных голосов.
fn voice_gap(embeddings: &[(String, Vec<f32>)]) -> f64 {
    let (mut same, mut other) = (Vec::new(), Vec::new());
    for (i, (va, a)) in embeddings.iter().enumerate() {
        for (vb, b) in &embeddings[i + 1..] {
            if va == vb {
                same.push(cosine(a, b));
            } else {
                other.push(cosine(a, b));
            }
        }
    }
    assert!(!same.is_empty() && !other.is_empty());
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    mean(&same) - mean(&other)
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn dynamic_redimnets_keep_one_voice_closer_than_others_on_pieces_of_one_and_a_half_three_and_six_seconds()
 {
    let dir = models_dir();
    let turns = turns_by_voice(&dir, &["ru2", "ru4", "ru6"]);
    for role in DYNAMIC_ROLES {
        let model = Redimnet2::load(&ort_path(&dir, role), 4).unwrap();
        for sec in [1.5, 3.0, 6.0] {
            let len = (sec * 16_000.0) as usize;
            let embeddings: Vec<(String, Vec<f32>)> = turns
                .iter()
                .filter(|(_, s)| s.len() >= len)
                .map(|(voice, s)| (voice.clone(), model.embed(&s[..len]).unwrap()))
                .collect();
            let gap = voice_gap(&embeddings);
            assert!(gap > 0.3, "{}, {sec} с: зазор {gap:.3}", role.key());
        }
    }
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn dynamic_b6_gives_the_same_embedding_as_the_six_second_export_on_a_six_second_piece() {
    let dir = models_dir();
    let fixed = redimnet(&dir);
    let dynamic = Redimnet2::load(&ort_path(&dir, OrtRole::Redimnet2B6Dyn), 4).unwrap();
    let samples = dialog_samples(&dir, "ru4");
    for start in [16_000 * 2, 16_000 * 30] {
        let piece = &samples[start..start + 96_000];
        let c = cosine(&fixed.embed(piece).unwrap(), &dynamic.embed(piece).unwrap());
        assert!(c > 0.9999, "косинус {c:.6}");
    }
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn dynamic_redimnet_gives_a_unit_embedding_for_a_tenth_of_a_second_and_none_for_a_tiny_piece() {
    let dir = models_dir();
    let model = Redimnet2::load(&ort_path(&dir, OrtRole::Redimnet2B0), 4).unwrap();
    let samples = dialog_samples(&dir, "ru2");
    let short = model.embed(&samples[16_000..17_600]).unwrap();
    assert_eq!(short.len(), model.dim());
    assert!((norm(&short) - 1.0).abs() < 1e-3, "норма {}", norm(&short));
    assert!(model.embed(&samples[..399]).unwrap().is_empty());
}

#[test]
#[ignore = "нужны основной набор, набор ort и диалоги, см. MODELS_DIR"]
fn pooled_redimnet_in_parallel_gives_the_embeddings_of_a_single_session() {
    let dir = models_dir();
    let samples = dialog_samples(&dir, "ru2");
    // Куски от 0,3 до 9 с: и повтор до 6 с, и разрез на окна по 6 с.
    let pieces: Vec<&[f32]> = (0..24)
        .map(|i| {
            let from = i * 16_000;
            let len = 4_800 + i * 6_000;
            &samples[from..(from + len).min(samples.len())]
        })
        .collect();
    let single = redimnet(&dir);
    let expected: Vec<Vec<f32>> = pieces.iter().map(|p| single.embed(p).unwrap()).collect();
    let pooled = Redimnet2::load_pool(&ort_path(&dir, OrtRole::Redimnet2), 4, 4).unwrap();
    let got = nixyon_core::par_map(&pieces, 4, |p| pooled.embed(p).unwrap());
    assert_eq!(got, expected);
}
