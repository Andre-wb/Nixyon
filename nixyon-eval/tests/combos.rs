//! Режим сочетаний eval даёт ту же расшифровку, что отдельный прогон каждого сочетания:
//! запись распознаётся один раз, а эмбеддинги и отрезки считаются на сочетание, в том
//! числе параллельно. Потоки на модель в обоих прогонах одинаковые. Нужны модели
//! (scripts/fetch-models.sh, набор ort) и диалоги (scripts/fetch-test-audio.sh):
//! `MODELS_DIR=models cargo test -p nixyon-eval --release -- --ignored`.

use std::path::{Path, PathBuf};

use nixyon_audio::{TARGET_SAMPLE_RATE, convert_to_wav16k};
use nixyon_core::{
    AudioInfo, CancelFlag, EngineInfo, NEURAL_BLOCK_SEC, SegmentResult, SpeechEngine,
    SpeechLanguage, VoiceSegmenter, VoiceSpan, to_json,
};
use nixyon_eval::{Combo, ComboModels, analyze_combos};
use nixyon_ort::OrtManifest;
use nixyon_pipeline::{
    PipelineOptions, SpeakerCount, TranscriptMeta, build_transcript_with_centers,
};
use nixyon_sherpa::{Decoding, ModelManifest, Parallel, SherpaEngine, SherpaModels};

// Sortformer fp32 в CI не качается, поэтому C здесь нет.
const COMBOS: [&str; 3] = [
    "A=campplus,pyannote,0.6",
    "B=redimnet2,pyannote,0.7",
    "D=campplus,sortformer-int8,0.6",
];
const THREADS: usize = 4;

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

fn base(dir: &Path, parallel: Parallel) -> SherpaModels {
    SherpaModels::load_parallel(
        &ModelManifest::load(dir).unwrap(),
        Decoding::default(),
        THREADS as i32,
        parallel,
    )
    .unwrap()
}

fn ort(dir: &Path) -> OrtManifest {
    OrtManifest::load(&dir.join("ort")).unwrap()
}

fn transcript(
    name: &str,
    combo: &Combo,
    segments: Vec<SegmentResult>,
    spans: &[VoiceSpan],
) -> String {
    let options = PipelineOptions {
        same_speaker_cosine: combo.same_speaker,
        ..PipelineOptions::default()
    };
    let meta = TranscriptMeta {
        created_at: String::new(),
        audio: AudioInfo {
            file: format!("{name}.wav"),
            duration_sec: 0.0,
            sample_rate: TARGET_SAMPLE_RATE,
        },
        engine: EngineInfo {
            vad: String::new(),
            asr: String::new(),
            embedding: String::new(),
        },
        language: SpeechLanguage::Ru,
    };
    let (t, _) = build_transcript_with_centers(
        segments,
        spans,
        &[],
        SpeakerCount::Auto,
        &options,
        meta,
        &CancelFlag::new(),
    )
    .unwrap();
    to_json(&t).unwrap()
}

// Как eval без сочетаний: модели с заменённой моделью голоса и нейросегментацией, затем
// разбор и отрезки по записи.
fn separate(dir: &Path, name: &str, combo: &Combo, wav: &Path) -> String {
    let ort = ort(dir);
    let models = combo
        .voice
        .apply(base(dir, Parallel::default()), &ort, THREADS, 1)
        .unwrap();
    let models = combo.segmenter.apply(models, &ort, THREADS).unwrap();
    let engine = SherpaEngine::new(models);
    let options = PipelineOptions::default().engine_options(SpeechLanguage::Ru);
    let segments = engine.analyze(wav, &options, &CancelFlag::new()).unwrap();
    let speech: Vec<(f64, f64)> = segments.iter().map(|s| (s.start, s.end)).collect();
    let spans = engine
        .voice_spans(wav, &speech, NEURAL_BLOCK_SEC, &CancelFlag::new())
        .unwrap();
    transcript(name, combo, segments, &spans)
}

fn together(
    dir: &Path,
    name: &str,
    combos: &[Combo],
    wav: &Path,
    parallel: Parallel,
) -> Vec<String> {
    let base = base(dir, parallel);
    let extra = ComboModels::load(combos, Some(&ort(dir)), THREADS, parallel.embed_jobs).unwrap();
    let results = analyze_combos(
        &base,
        &extra,
        combos,
        wav,
        &PipelineOptions::default().engine_options(SpeechLanguage::Ru),
        Some(NEURAL_BLOCK_SEC),
        &CancelFlag::new(),
    )
    .unwrap();
    combos
        .iter()
        .zip(results)
        .map(|(combo, r)| transcript(name, combo, r.segments, &r.spans))
        .collect()
}

// Первое расхождение двух JSON с окрестностью: по нему видно, где и насколько разошлись.
fn first_difference(a: &str, b: &str) -> Option<String> {
    let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y);
    let at = match at {
        Some(at) => at,
        None if a.len() == b.len() => return None,
        None => a.len().min(b.len()),
    };
    let around = |s: &str| {
        let from = s.floor_char_boundary(at.saturating_sub(200));
        let to = s.ceil_char_boundary((at + 200).min(s.len()));
        s[from..to].to_string()
    };
    Some(format!(
        "байт {at}\nотдельно:\n{}\nвместе:\n{}",
        around(a),
        around(b)
    ))
}

#[test]
#[ignore = "нужны модели, набор ort и диалоги, см. MODELS_DIR"]
fn combos_give_the_same_transcript_as_separate_runs_also_when_work_goes_in_parallel() {
    let dir = models_dir();
    let combos: Vec<Combo> = COMBOS.iter().map(|c| Combo::parse(c).unwrap()).collect();
    for name in ["ru2", "ru4_overlap"] {
        let wav =
            std::env::temp_dir().join(format!("nixyon-combos-{}-{name}.wav", std::process::id()));
        convert_to_wav16k(&dir.join(format!("dialogs/{name}.wav")), &wav).unwrap();
        let expected: Vec<String> = combos
            .iter()
            .map(|c| separate(&dir, name, c, &wav))
            .collect();
        // Всё по одному и всё параллельно: эмбеддинги в 4 потока, распознавание в 3.
        let variants = [
            Parallel::default(),
            Parallel {
                embed_jobs: 4,
                asr_jobs: 3,
            },
        ];
        for parallel in variants {
            let got = together(&dir, name, &combos, &wav, parallel);
            for ((combo, want), have) in combos.iter().zip(&expected).zip(&got) {
                if let Some(diff) = first_difference(want, have) {
                    panic!("{name}, сочетание {}, {parallel:?}: {diff}", combo.name);
                }
            }
        }
        std::fs::remove_file(&wav).unwrap();
    }
}
