//! Разделение спикеров на живых записях. Нужны модели (scripts/fetch-models.sh) и
//! диалоги (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-sherpa --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_audio::convert_to_wav16k;
use nixyon_core::{AudioInfo, CancelFlag, EngineInfo, SegmentResult, Transcript, VoiceSpan};
use nixyon_pipeline::{PipelineOptions, SpeakerCount, TranscriptMeta, build_transcript};
use nixyon_sherpa::{ModelManifest, SherpaEngine, SherpaModels, analyze_default};
use serde_json::Value;

const MIN_ACCURACY: f64 = 0.95;

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

fn meta() -> TranscriptMeta {
    TranscriptMeta {
        created_at: "2026-10-28T00:00:00Z".into(),
        audio: AudioInfo {
            file: "test.wav".into(),
            duration_sec: 0.0,
            sample_rate: 16_000,
        },
        engine: EngineInfo {
            vad: "vad".into(),
            asr: "asr".into(),
            embedding: "embedding".into(),
        },
    }
}

/// Сравнение расшифровки с разметкой: доля реплик, попавших в расшифровку
/// (перекрытых хоть одним ходом), и доля верно приписанных среди них. Спикер реплики:
/// тот, чьи ходы больше всего перекрывают её; номера сопоставляются один к одному.
fn coverage_and_accuracy(transcript: &Transcript, truth: &[Value]) -> (f64, f64) {
    let mut votes: Vec<(u64, u32)> = Vec::new();
    for t in truth {
        let (start, end) = (t["start"].as_f64().unwrap(), t["end"].as_f64().unwrap());
        let overlap = |x: &nixyon_core::Turn| (end.min(x.end) - start.max(x.start)).max(0.0);
        let best = transcript
            .turns
            .iter()
            .max_by(|a, b| overlap(a).total_cmp(&overlap(b)));
        if let Some(turn) = best.filter(|turn| overlap(turn) > 0.0) {
            votes.push((t["speaker"].as_u64().unwrap(), turn.speaker));
        }
    }
    let mut pairs: Vec<((u64, u32), usize)> = Vec::new();
    for v in &votes {
        match pairs.iter_mut().find(|(p, _)| p == v) {
            Some((_, n)) => *n += 1,
            None => pairs.push((*v, 1)),
        }
    }
    pairs.sort_by_key(|pair| std::cmp::Reverse(pair.1));
    let (mut used_truth, mut used_pred, mut matched) = (Vec::new(), Vec::new(), 0);
    for ((t, p), n) in pairs {
        if !used_truth.contains(&t) && !used_pred.contains(&p) {
            used_truth.push(t);
            used_pred.push(p);
            matched += n;
        }
    }
    let covered = votes.len() as f64;
    (
        covered / truth.len() as f64,
        matched as f64 / covered.max(1.0),
    )
}

/// Тот же путь звука, что в приложении: перевод в 16 кГц с выравниванием громкости, затем
/// движок с теми же настройками, что у приложения (куски и отрезки нейросегментации).
fn analyze(engine: &SherpaEngine, wav: &Path) -> (Vec<SegmentResult>, Vec<VoiceSpan>) {
    let name = wav.file_stem().unwrap().to_string_lossy();
    let converted =
        std::env::temp_dir().join(format!("nixyon-dialogs-{}-{name}.wav", std::process::id()));
    convert_to_wav16k(wav, &converted).unwrap();
    let analyzed = analyze_default(
        engine,
        &converted,
        &PipelineOptions::default(),
        &CancelFlag::new(),
    )
    .unwrap();
    std::fs::remove_file(&converted).unwrap();
    analyzed
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn speakers_are_found_and_separated_on_real_recordings() {
    let dir = models_dir();
    let engine =
        SherpaEngine::new(SherpaModels::load(&ModelManifest::load(&dir).unwrap()).unwrap());

    let (example, example_spans) = analyze(&engine, &dir.join("example.wav"));
    let t = build_transcript(
        example,
        &example_spans,
        &[],
        SpeakerCount::Auto,
        &PipelineOptions::default(),
        meta(),
        &CancelFlag::new(),
    )
    .unwrap();
    assert_eq!(t.speakers.len(), 1, "example.wav: один чтец");

    // Только русские записи: расшифровка держит лишь куски с текстом, а на китайской
    // real3 GigaAM текста почти не даёт. Разделение голосов на всех кусках, включая
    // real3, проверяет nixyon-eval/tests/diarization.rs.
    for name in ["ru2", "ru4", "ru6"] {
        let truth: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join(format!("dialogs/{name}.truth.json"))).unwrap(),
        )
        .unwrap();
        let people = truth["speakers"].as_u64().unwrap();
        let turns = truth["turns"].as_array().unwrap();
        let (segments, spans) = analyze(&engine, &dir.join(format!("dialogs/{name}.wav")));

        let auto = build_transcript(
            segments.clone(),
            &spans,
            &[],
            SpeakerCount::Auto,
            &PipelineOptions::default(),
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        assert_eq!(auto.speakers.len() as u64, people, "{name}: число спикеров");
        let (coverage, auto_acc) = coverage_and_accuracy(&auto, turns);
        assert!(
            coverage >= MIN_ACCURACY,
            "{name}: в расшифровку попало {coverage:.2} реплик"
        );
        assert!(
            auto_acc >= MIN_ACCURACY,
            "{name}: точность без числа спикеров {auto_acc:.2}"
        );

        let known = SpeakerCount::Known(u32::try_from(people).unwrap());
        let given = build_transcript(
            segments,
            &spans,
            &[],
            known,
            &PipelineOptions::default(),
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        let (_, given_acc) = coverage_and_accuracy(&given, turns);
        assert!(
            given_acc >= MIN_ACCURACY,
            "{name}: точность с числом спикеров {given_acc:.2}"
        );
        println!(
            "{name}: спикеров {people}, в расшифровке {coverage:.2} реплик, точность {auto_acc:.2} без числа, {given_acc:.2} с числом"
        );
    }
}
