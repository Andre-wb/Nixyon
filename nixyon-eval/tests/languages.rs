//! Язык речи: определение языка Whisper по записи и распознавание английской речи Parakeet.
//! Нужны модели (scripts/fetch-models.sh) и диалоги (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-eval --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_audio::{TARGET_SAMPLE_RATE, convert_to_wav16k};
use nixyon_core::{AudioInfo, CancelFlag, EngineInfo, SpeechLanguage};
use nixyon_eval::hypothesis_text;
use nixyon_metrics::word_errors;
use nixyon_pipeline::{PipelineOptions, SpeakerCount, TranscriptMeta, build_transcript};
use nixyon_sherpa::{
    ModelManifest, SherpaEngine, SherpaModels, analyze_default, detect_wav_language,
};
use serde_json::Value;

/// На en2 замерено 4,0 % (жадный поиск); поиск по лучам терял фразы и давал 27 %.
const MAX_ENGLISH_WER: f64 = 0.10;

fn models_dir() -> PathBuf {
    let dir = PathBuf::from(
        std::env::var("MODELS_DIR").expect("задайте MODELS_DIR: папку с models.json"),
    );
    if dir.is_absolute() {
        dir
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(dir)
    }
}

fn engine(dir: &Path) -> SherpaEngine {
    SherpaEngine::new(SherpaModels::load(&ModelManifest::load(dir).unwrap()).unwrap())
}

// Перевод в 16 кГц тем же путём, что в приложении; временный файл удаляется.
fn with_wav16k<T>(wav: &Path, work: impl FnOnce(&Path) -> T) -> T {
    let name = wav.file_stem().unwrap().to_string_lossy();
    let converted = std::env::temp_dir().join(format!(
        "nixyon-languages-{}-{name}.wav",
        std::process::id()
    ));
    convert_to_wav16k(wav, &converted).unwrap();
    let result = work(&converted);
    std::fs::remove_file(&converted).unwrap();
    result
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn language_of_russian_and_english_dialogs_is_detected() {
    let dir = models_dir();
    let engine = engine(&dir);
    let options = PipelineOptions::default().engine_options(SpeechLanguage::Ru);
    for (name, expected) in [
        ("ru2", SpeechLanguage::Ru),
        ("ru4", SpeechLanguage::Ru),
        ("ru6", SpeechLanguage::Ru),
        ("en2", SpeechLanguage::En),
        ("en4", SpeechLanguage::En),
        ("en6", SpeechLanguage::En),
    ] {
        let wav = dir.join(format!("dialogs/{name}.wav"));
        let found = with_wav16k(&wav, |wav| {
            detect_wav_language(engine.models(), wav, &options, &CancelFlag::new()).unwrap()
        });
        assert_eq!(found, expected, "{name}");
    }
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn english_dialog_is_recognized_by_parakeet() {
    let dir = models_dir();
    let engine = engine(&dir);
    let wav = dir.join("dialogs/en2.wav");
    let truth: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("dialogs/en2.truth.json")).unwrap())
            .unwrap();
    let reference: Vec<&str> = truth["turns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["text"].as_str().unwrap())
        .collect();

    let options = PipelineOptions::default();
    let (segments, spans) = with_wav16k(&wav, |wav| {
        analyze_default(
            &engine,
            wav,
            &options,
            SpeechLanguage::En,
            &CancelFlag::new(),
        )
        .unwrap()
    });
    let meta = TranscriptMeta {
        created_at: String::new(),
        audio: AudioInfo {
            file: "en2.wav".into(),
            duration_sec: 0.0,
            sample_rate: TARGET_SAMPLE_RATE,
        },
        engine: EngineInfo {
            vad: String::new(),
            asr: String::new(),
            embedding: String::new(),
        },
        language: SpeechLanguage::En,
    };
    let transcript = build_transcript(
        segments,
        &spans,
        &[],
        SpeakerCount::Auto,
        &options,
        meta,
        &CancelFlag::new(),
    )
    .unwrap();
    assert_eq!(transcript.language, SpeechLanguage::En);
    assert_eq!(transcript.speakers.len(), 2, "en2: два человека");
    let errors = word_errors(&reference.join(" "), &hypothesis_text(&transcript));
    let wer = errors.errors as f64 / errors.reference_len as f64;
    assert!(wer <= MAX_ENGLISH_WER, "WER en2 {wer:.3}");
}
