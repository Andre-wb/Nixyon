//! Прогон расшифровки на ноутбуке:
//! `transcribe <папка моделей> <файл.wav> <расшифровка.json> [число спикеров]`.
//! WAV любой поддерживаемой частоты сначала переводится в 16 кГц моно. Язык речи (русский
//! или английский) определяется по записи, как в приложении в режиме «Авто».

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use nixyon_audio::{TARGET_SAMPLE_RATE, convert_to_wav16k};
use nixyon_core::{AudioInfo, CancelFlag, EngineInfo, SpeechLanguage, rfc3339_utc, to_json};
use nixyon_pipeline::{PipelineOptions, SpeakerCount, TranscriptMeta, build_transcript};
use nixyon_sherpa::{
    ModelManifest, ModelRole, SherpaEngine, SherpaModels, analyze_default, detect_wav_language,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (models_dir, wav, out, count) = match args.as_slice() {
        [_, m, w, o] => (m, w, o, SpeakerCount::Auto),
        [_, m, w, o, n] => match n.parse::<u32>() {
            Ok(n) if n > 0 => (m, w, o, SpeakerCount::Known(n)),
            _ => {
                eprintln!("число спикеров: целое больше нуля");
                return ExitCode::from(2);
            }
        },
        _ => {
            eprintln!(
                "использование: transcribe <папка моделей> <файл.wav> <расшифровка.json> [число спикеров]"
            );
            return ExitCode::from(2);
        }
    };
    match run(Path::new(models_dir), Path::new(wav), Path::new(out), count) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ошибка: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    models_dir: &Path,
    wav: &Path,
    out: &Path,
    count: SpeakerCount,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = ModelManifest::load(models_dir)?;
    let engine = SherpaEngine::new(SherpaModels::load(&manifest)?);

    let wav_16k: PathBuf =
        std::env::temp_dir().join(format!("nixyon-transcribe-{}.wav", std::process::id()));
    let report = convert_to_wav16k(wav, &wav_16k)?;
    println!(
        "запись: {:.2} с, самое громкое окно {:.1} дБ, клиппинг {:.4} %, замечания {:?}",
        report.duration_sec,
        report.loudest_window_dbfs,
        report.clipped_ratio * 100.0,
        report.issues()
    );
    let options = PipelineOptions::default();
    let cancel = CancelFlag::new();
    // Куски речи от языка не зависят, язык в опциях здесь любой.
    let analyzed = detect_wav_language(
        engine.models(),
        &wav_16k,
        &options.engine_options(SpeechLanguage::Ru),
        &cancel,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })
    .and_then(|language| {
        println!("язык речи: {}", language.code());
        let analyzed = analyze_default(&engine, &wav_16k, &options, language, &cancel)?;
        Ok((language, analyzed))
    });
    std::fs::remove_file(&wav_16k)?;
    let (language, (segments, spans)) = analyzed?;

    let meta = TranscriptMeta {
        created_at: utc_now_rfc3339(),
        audio: AudioInfo {
            file: wav
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            duration_sec: report.duration_sec,
            sample_rate: TARGET_SAMPLE_RATE,
        },
        engine: EngineInfo {
            vad: file_name(&manifest.path(ModelRole::Vad)),
            asr: manifest.version().to_string(),
            embedding: file_name(&manifest.path(ModelRole::Embedding)),
        },
        language,
    };
    let transcript = build_transcript(
        segments,
        &spans,
        &[],
        count,
        &PipelineOptions::default(),
        meta,
        &CancelFlag::new(),
    )?;
    std::fs::write(out, to_json(&transcript)?)?;

    for turn in &transcript.turns {
        println!(
            "[{:6.2}-{:6.2}] Спикер {}: {}",
            turn.start, turn.end, turn.speaker, turn.text
        );
    }
    println!(
        "спикеров: {}, ходов: {}",
        transcript.speakers.len(),
        transcript.turns.len()
    );
    Ok(())
}

// В расшифровку попадает только имя файла модели: полный путь раскрыл бы папки пользователя.
fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn utc_now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339_utc(secs)
}
