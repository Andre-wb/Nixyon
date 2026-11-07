//! Края речи: клипы Common Voice (CC0), где VAD не находил речи вовсе или резал кусок
//! вплотную к словам, и модель теряла первое и последнее слово или всю фразу. Весь
//! конвейер приложения, как в `quality_eval`. Нужны модели (scripts/fetch-models.sh):
//! `MODELS_DIR=models cargo test -p nixyon-eval --release -- --ignored`.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use nixyon_audio::{TARGET_SAMPLE_RATE, WavWriter, convert_reader_to_wav16k};
use nixyon_core::{AudioInfo, CancelFlag, EngineInfo, SpeechLanguage};
use nixyon_eval::{CvClipFiles, decode_mp3_mono, hypothesis_text, list_cv_clips};
use nixyon_metrics::word_errors;
use nixyon_pipeline::{PipelineOptions, SpeakerCount, TranscriptMeta, build_transcript};
use nixyon_sherpa::{ModelManifest, SherpaEngine, SherpaModels, analyze_default};

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

fn clip(id: &str) -> CvClipFiles {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    list_cv_clips(&dir)
        .unwrap()
        .into_iter()
        .find(|c| c.clip.id == id)
        .unwrap()
}

fn recognize(engine: &SherpaEngine, files: &CvClipFiles, wav: &Path) -> String {
    let audio = decode_mp3_mono(&files.mp3).unwrap();
    let mut writer = WavWriter::new(Cursor::new(Vec::new()), audio.sample_rate).unwrap();
    writer.write(&audio.samples).unwrap();
    let bytes = writer.finish(audio.sample_rate).unwrap().into_inner();
    let converted = convert_reader_to_wav16k(Cursor::new(bytes), wav).unwrap();
    let options = PipelineOptions::default();
    let cancel = CancelFlag::new();
    let (segments, spans) =
        analyze_default(engine, wav, &options, SpeechLanguage::Ru, &cancel).unwrap();
    let meta = TranscriptMeta {
        created_at: String::new(),
        audio: AudioInfo {
            file: String::new(),
            duration_sec: converted.duration_sec,
            sample_rate: TARGET_SAMPLE_RATE,
        },
        engine: EngineInfo {
            vad: String::new(),
            asr: String::new(),
            embedding: String::new(),
        },
        language: SpeechLanguage::Ru,
    };
    let transcript = build_transcript(
        segments,
        &spans,
        &[],
        SpeakerCount::Auto,
        &options,
        meta,
        &cancel,
    )
    .unwrap_or_else(|e| panic!("{}: {e}", files.clip.id));
    hypothesis_text(&transcript)
}

// Было: пустой текст, пустой текст, «Изменит.», «Стоял, не снимая пальто. Тамана.».
// Было ошибок 9, 1, 10 и 7.
#[test]
#[ignore = "нужны модели, см. MODELS_DIR"]
fn quiet_words_and_the_edges_of_a_phrase_are_not_lost() {
    let engine = SherpaEngine::new(
        SherpaModels::load(&ModelManifest::load(&models_dir()).unwrap()).unwrap(),
    );
    let wav = std::env::temp_dir().join(format!("nixyon-edges-{}.wav", std::process::id()));
    for (id, max_errors) in [
        ("common_voice_ru_21314344", 1),
        ("common_voice_ru_21649796", 0),
        ("common_voice_ru_33165532", 2),
        ("common_voice_ru_26316630", 3),
    ] {
        let files = clip(id);
        let text = recognize(&engine, &files, &wav);
        let errors = word_errors(&files.clip.text, &text).errors;
        assert!(
            errors <= max_errors,
            "{id}: ошибок {errors}, распознано «{text}», эталон «{}»",
            files.clip.text
        );
    }
    let _ = std::fs::remove_file(&wav);
}
