//! Живой текст: потоковая модель из набора моделей распознаёт запись, поданную кусками
//! как с микрофона, и текст появляется до конца записи. Нужны модели
//! (scripts/fetch-models.sh) и диалоги (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-eval --release -- --ignored`.

use std::path::PathBuf;

use nixyon_core::SpeechLanguage;
use nixyon_eval::Truth;
use nixyon_metrics::word_errors;
use nixyon_sherpa::{LiveText, ModelManifest};
use sherpa_onnx::Wave;

// На ru2 при замере 9,6 %, на en2 у английской модели 9,1 %; запас на разницу сборок
// sherpa-onnx.
const MAX_WER: f64 = 0.12;

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

// Подаёт диалог кусками по 0,1 с, как микрофон, и проверяет, что текст появился до середины
// записи, разбит на фразы и близок к эталону.
fn live_text_follows_the_recording(language: SpeechLanguage, name: &str) {
    let dir = models_dir();
    let model = ModelManifest::load(&dir)
        .unwrap()
        .live_model(language)
        .expect("в models.json нет живой модели этого языка");
    let wave = Wave::read(&dir.join(format!("dialogs/{name}.wav")).to_string_lossy()).unwrap();
    let rate = wave.sample_rate();
    let truth = Truth::from_json(
        &std::fs::read_to_string(dir.join(format!("dialogs/{name}.truth.json"))).unwrap(),
    )
    .unwrap();

    let mut live = LiveText::new(&model, 1, rate).unwrap();
    let chunk = usize::try_from(rate / 10).unwrap();
    let pieces: Vec<&[f32]> = wave.samples().chunks(chunk).collect();
    let half = pieces.len() / 2;
    let mut text_before_half = false;
    for (i, piece) in pieces.into_iter().enumerate() {
        live.accept(rate, piece);
        if i < half && !live.partial().is_empty() {
            text_before_half = true;
        }
    }
    let phrases = live.finish(rate);

    assert!(
        text_before_half,
        "{name}: до середины записи текста не было"
    );
    assert!(
        phrases.len() > 1,
        "{name}: запись не разбилась на фразы: {}",
        phrases.len()
    );
    let wer = word_errors(&truth.text().unwrap(), &phrases.join(" "))
        .rate()
        .unwrap();
    println!("{name}: живой текст WER {:.1} %", wer * 100.0);
    assert!(wer <= MAX_WER, "{name}: WER {wer:.3} больше {MAX_WER}");
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn live_model_from_the_model_set_writes_text_while_the_recording_goes() {
    live_text_follows_the_recording(SpeechLanguage::Ru, "ru2");
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn english_live_model_writes_english_text_while_the_recording_goes() {
    live_text_follows_the_recording(SpeechLanguage::En, "en2");
}
