//! Разделение голосов на живых записях без участия текста: размечаются все куски
//! речи, в том числе те, где модель не дала текста. Нужны модели
//! (scripts/fetch-models.sh) и диалоги (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-eval --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_audio::convert_to_wav16k;
use nixyon_core::{CancelFlag, EngineOptions, SegmentResult, SpeechEdges, SpeechEngine};
use nixyon_eval::Truth;
use nixyon_metrics::{DER_COLLAR_SEC, DerBreakdown, SpeakerSpan, diarization_errors};
use nixyon_pipeline::{PipelineOptions, SpeakerCount, label_segments};
use nixyon_sherpa::{ModelManifest, SherpaEngine, SherpaModels};

/// Одна неверно приписанная реплика самой короткой длины за вычетом воротника
/// даёт на этих записях от 1,9 % путаницы, порог ниже.
const MAX_CONFUSION: f64 = 0.01;
/// Пропуск и ложная речь зависят от VAD; порог ловит выпадение кусков без текста
/// (тогда DER на real3 выше 50 %).
const MAX_DER: f64 = 0.25;

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

/// Тот же путь звука, что в приложении: перевод в 16 кГц с выравниванием громкости, затем
/// движок с теми же окнами, что просит приложение. Куски идут по времени.
fn analyze(engine: &SherpaEngine, wav: &Path) -> Vec<SegmentResult> {
    let name = wav.file_stem().unwrap().to_string_lossy();
    let converted = std::env::temp_dir().join(format!(
        "nixyon-diarization-{}-{name}.wav",
        std::process::id()
    ));
    convert_to_wav16k(wav, &converted).unwrap();
    let mut segments = engine
        .analyze(
            &converted,
            // Деление на спикеров по кускам VAD просит строгие края, как `eval --no-neural`.
            &EngineOptions {
                edges: SpeechEdges::Tight,
                ..PipelineOptions::default().engine_options()
            },
            &CancelFlag::new(),
        )
        .unwrap();
    std::fs::remove_file(&converted).unwrap();
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    segments
}

/// Каждый кусок целиком отдаётся своему спикеру: разрез по словам кускам без текста
/// недоступен, поэтому проверяется разметка кусков, а не итоговые ходы.
fn hypothesis(segments: &[SegmentResult], speakers: &[u32]) -> Vec<SpeakerSpan> {
    segments
        .iter()
        .zip(speakers)
        .map(|(s, &speaker)| SpeakerSpan {
            speaker,
            start: s.start,
            end: s.end,
        })
        .collect()
}

fn distinct(speakers: &[u32]) -> usize {
    let mut ids = speakers.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids.len()
}

fn share(part: f64, d: &DerBreakdown) -> f64 {
    part / d.reference_speech
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn voices_are_separated_on_all_speech_segments_regardless_of_text() {
    let dir = models_dir();
    let engine =
        SherpaEngine::new(SherpaModels::load(&ModelManifest::load(&dir).unwrap()).unwrap());

    for name in ["real3", "ru2", "ru4", "ru6"] {
        let truth = Truth::from_json(
            &std::fs::read_to_string(dir.join(format!("dialogs/{name}.truth.json"))).unwrap(),
        )
        .unwrap();
        let segments = analyze(&engine, &dir.join(format!("dialogs/{name}.wav")));
        let without_text = segments.iter().filter(|s| s.text.trim().is_empty()).count();

        for (mode, count) in [
            ("без числа", SpeakerCount::Auto),
            ("с числом", SpeakerCount::Known(truth.speakers)),
        ] {
            let speakers = label_segments(&segments, count, &PipelineOptions::default()).segments;
            let found = distinct(&speakers);
            let d = diarization_errors(
                &truth.spans(),
                &hypothesis(&segments, &speakers),
                DER_COLLAR_SEC,
            )
            .unwrap();
            let der = d.rate().unwrap();
            let confusion = share(d.confusion, &d);
            println!(
                "{name} {mode}: кусков {}, без текста {without_text}, спикеров {found} из {}, DER {der:.3} (пропуск {:.3}, ложная {:.3}, путаница {confusion:.3})",
                segments.len(),
                truth.speakers,
                share(d.missed, &d),
                share(d.false_alarm, &d),
            );
            assert_eq!(
                found, truth.speakers as usize,
                "{name} {mode}: число спикеров"
            );
            assert!(
                confusion <= MAX_CONFUSION,
                "{name} {mode}: путаница {confusion:.3}"
            );
            assert!(der <= MAX_DER, "{name} {mode}: DER {der:.3}");
        }
    }
}
