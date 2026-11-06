//! Живой текст: потоковая модель из набора моделей распознаёт запись, поданную кусками
//! как с микрофона, и текст появляется до конца записи. Нужны модели
//! (scripts/fetch-models.sh) и диалоги (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-eval --release -- --ignored`.

use std::path::PathBuf;

use nixyon_eval::Truth;
use nixyon_metrics::word_errors;
use nixyon_sherpa::{LiveText, ModelManifest};
use sherpa_onnx::Wave;

// На ru2 при замере 9,6 %; запас на разницу сборок sherpa-onnx.
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

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn live_model_from_the_model_set_writes_text_while_the_recording_goes() {
    let dir = models_dir();
    let model = ModelManifest::load(&dir)
        .unwrap()
        .live_model()
        .expect("в models.json нет живой модели");
    let wave = Wave::read(&dir.join("dialogs/ru2.wav").to_string_lossy()).unwrap();
    let rate = wave.sample_rate();
    let truth =
        Truth::from_json(&std::fs::read_to_string(dir.join("dialogs/ru2.truth.json")).unwrap())
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

    assert!(text_before_half, "до середины записи текста не было");
    assert!(
        phrases.len() > 1,
        "запись не разбилась на фразы: {}",
        phrases.len()
    );
    let wer = word_errors(&truth.text().unwrap(), &phrases.join(" "))
        .rate()
        .unwrap();
    println!("ru2: живой текст WER {:.1} %", wer * 100.0);
    assert!(wer <= MAX_WER, "WER {wer:.3} больше {MAX_WER}");
}
