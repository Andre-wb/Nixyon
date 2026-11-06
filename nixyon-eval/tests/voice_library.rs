//! Библиотека голосов: голоса, запомненные на двух прошлых встречах (prior6a, prior6b,
//! клипы не пересекаются с диалогами), сами дают имена на новых записях. Модель голоса и
//! порог слияния групп как в приложении: ReDimNet2 и 0,7. Нужны модели
//! (scripts/fetch-models.sh, набор ort) и диалоги (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-eval --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_audio::convert_to_wav16k;
use nixyon_core::{
    AudioInfo, CancelFlag, EngineInfo, NEURAL_BLOCK_SEC, SegmentResult, SpeechEngine, VoiceSample,
    VoiceSegmenter, VoiceSpan,
};
use nixyon_eval::{RecordRun, Truth, VoiceModel, library_from_meeting, score_record};
use nixyon_ort::{OrtManifest, OrtRole};
use nixyon_pipeline::{
    PipelineOptions, SpeakerCount, TranscriptMeta, build_transcript_with_centers,
};
use nixyon_sherpa::{ModelManifest, SherpaEngine, SherpaModels};

// Порог слияния групп для ReDimNet2, как VoiceChoice.SAME_SPEAKER_COSINE в Kotlin-плагине.
const SAME_SPEAKER_COSINE: f64 = 0.7;
const THREADS: usize = 1;

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

/// Путь звука как в приложении: 16 кГц с выравниванием громкости, куски речи и отрезки
/// нейросегментации.
fn analyze(engine: &SherpaEngine, wav: &Path) -> (Vec<SegmentResult>, Vec<VoiceSpan>, f64) {
    let name = wav.file_stem().unwrap().to_string_lossy();
    let converted =
        std::env::temp_dir().join(format!("nixyon-library-{}-{name}.wav", std::process::id()));
    let report = convert_to_wav16k(wav, &converted).unwrap();
    let options = PipelineOptions::default();
    let segments = engine
        .analyze(&converted, &options.engine_options(), &CancelFlag::new())
        .unwrap();
    let speech: Vec<(f64, f64)> = segments.iter().map(|s| (s.start, s.end)).collect();
    let spans = engine
        .voice_spans(&converted, &speech, NEURAL_BLOCK_SEC, &CancelFlag::new())
        .unwrap();
    std::fs::remove_file(&converted).unwrap();
    (segments, spans, report.duration_sec)
}

// Модели набора с ReDimNet2 вместо CAM++, сессий модели голоса по числу ядер.
fn engine(dir: &Path) -> SherpaEngine {
    let jobs = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ort = OrtManifest::load(&dir.join("ort")).unwrap();
    let models = SherpaModels::load(&ModelManifest::load(dir).unwrap()).unwrap();
    SherpaEngine::new(
        VoiceModel::Redimnet2(OrtRole::Redimnet2)
            .apply(models, &ort, THREADS, jobs)
            .unwrap(),
    )
}

fn meta(name: &str, duration_sec: f64) -> TranscriptMeta {
    TranscriptMeta {
        created_at: String::new(),
        audio: AudioInfo {
            file: format!("{name}.wav"),
            duration_sec,
            sample_rate: 16_000,
        },
        engine: EngineInfo {
            vad: String::new(),
            asr: String::new(),
            embedding: String::new(),
        },
    }
}

fn truth(dir: &Path, name: &str) -> Truth {
    Truth::from_json(
        &std::fs::read_to_string(dir.join(format!("dialogs/{name}.truth.json"))).unwrap(),
    )
    .unwrap()
}

#[test]
#[ignore = "нужны модели и диалоги, см. MODELS_DIR"]
fn voices_remembered_at_two_past_meetings_name_speakers_of_new_recordings() {
    let dir = models_dir();
    let engine = engine(&dir);
    let options = PipelineOptions {
        same_speaker_cosine: SAME_SPEAKER_COSINE,
        ..PipelineOptions::default()
    };

    let mut library: Vec<VoiceSample> = Vec::new();
    for name in ["prior6a", "prior6b"] {
        let (segments, spans, duration) =
            analyze(&engine, &dir.join(format!("dialogs/{name}.wav")));
        let (transcript, centers) = build_transcript_with_centers(
            segments,
            &spans,
            &[],
            SpeakerCount::Auto,
            &options,
            meta(name, duration),
            &CancelFlag::new(),
        )
        .unwrap();
        library.extend(library_from_meeting(
            &transcript,
            &centers,
            &truth(&dir, name),
        ));
    }
    let mut names: Vec<String> = library.iter().map(|s| s.name.clone()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 6, "в библиотеке все шесть человек: {names:?}");

    for name in ["ru2", "ru4", "ru6", "real3"] {
        let reference = truth(&dir, name);
        let (segments, spans, duration) =
            analyze(&engine, &dir.join(format!("dialogs/{name}.wav")));
        let (transcript, _) = build_transcript_with_centers(
            segments,
            &spans,
            &library,
            SpeakerCount::Auto,
            &options,
            meta(name, duration),
            &CancelFlag::new(),
        )
        .unwrap();
        let score = score_record(&RecordRun {
            name,
            truth: &reference,
            transcript: &transcript,
            audio_sec: duration,
            processing_sec: 0.0,
            samples: Some(&names),
        })
        .unwrap();
        let naming = score.naming.unwrap();
        println!(
            "{name}: имён нужно {}, верно {}, ошибочно {}",
            naming.expected, naming.correct, naming.wrong
        );
        assert_eq!(naming.wrong, 0, "{name}: чужие имена");
        assert_eq!(naming.missed(), 0, "{name}: пропущенные имена");
    }
}
