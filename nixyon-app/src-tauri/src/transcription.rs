//! Обработка записи на устройстве: из файла собирается расшифровка. Звук готовит
//! ядро (nixyon-audio), куски речи, текст и голоса даёт Kotlin-плагин, спикеров и
//! расшифровку собирает конвейер (nixyon-pipeline).

pub(crate) mod allowed_path;
mod command_error;
mod progress;
mod quality;
mod running;
mod same_speaker;
mod speaker_ages;
pub(crate) mod temp_files;
mod turn_emotions;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use nixyon_android::{
    AnalyzeProgress, AnalyzeRequest, AnalyzeSummary, SpeechExt, read_overlaps, read_segments,
    read_spans,
};
use nixyon_audio::{TARGET_SAMPLE_RATE, convert_reader_to_wav16k};
use nixyon_core::{
    AudioInfo, EngineInfo, ModelOverlaps, NEURAL_BLOCK_SEC, Transcript, hotwords_line, rfc3339_utc,
};
use nixyon_pipeline::{
    MIN_SILENT_OVERLAP_SEC, PipelineError, PipelineOptions, Segmentation, SpeakerCount,
    TranscriptMeta, build_transcript_from,
};
use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime, State};

use crate::meetings::{NewMeeting, save_meeting};

pub use command_error::CommandError;
pub use progress::{Phase, Progress};
pub use running::{Running, Slot};

use crate::settings::merge_hotwords;
use temp_files::TempFiles;

#[derive(Serialize)]
pub struct TranscribeResult {
    pub transcript: Transcript,
    /// Предупреждения о записи, например "clipping".
    pub warnings: Vec<&'static str>,
}

/// Расшифровка записи. `speakers`: число участников или None, если пользователь не знает.
#[tauri::command]
pub async fn transcribe<R: Runtime>(
    app: AppHandle<R>,
    running: State<'_, Running>,
    path: String,
    speakers: Option<u32>,
    name: Option<String>,
    on_progress: Channel<Progress>,
) -> Result<TranscribeResult, CommandError> {
    let count = match speakers {
        None => SpeakerCount::Auto,
        Some(0) => return Err(CommandError::new(CommandError::BAD_SPEAKERS)),
        Some(n) => SpeakerCount::Known(n),
    };
    let slot = running
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;
    let cancel = slot.cancel.clone();
    let speech = app.speech();
    let storage = crate::vault::storage(&app).await?;
    let (store, dirs) = (storage.store, storage.paths);
    // Встреча забирает запись к себе в папку, поэтому принимаются только записи и импорт
    // приложения из `recordings`.
    let roots = [Path::new(&dirs.files).join(crate::vault::RECORDINGS_DIR)];
    let input = allowed_path::resolve_input(&path, &roots).ok_or_else(|| {
        log::warn!("путь вне папок приложения или не файл");
        CommandError::new(CommandError::BAD_PATH)
    })?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let cache = Path::new(&dirs.cache);
    let wav = cache.join(format!("nixyon-{stamp}.wav"));
    let segments_file = cache.join(format!("nixyon-{stamp}.segments.json"));
    let spans_file = cache.join(format!("nixyon-{stamp}.spans.json"));
    let overlaps_file = cache.join(format!("nixyon-{stamp}.overlaps.json"));
    let _temp = TempFiles::new(vec![
        wav.clone(),
        segments_file.clone(),
        spans_file.clone(),
        overlaps_file.clone(),
    ]);

    let _ = on_progress.send(Progress {
        phase: Phase::Prepare,
        done_sec: 0.0,
        total_sec: 0.0,
    });
    let (input_for_convert, wav_for_convert) = (input.clone(), wav.clone());
    let key = std::sync::Arc::clone(store.key());
    let report = tauri::async_runtime::spawn_blocking(move || {
        let source = crate::vault::open_read(&input_for_convert, &key)?;
        convert_reader_to_wav16k(source, &wav_for_convert)
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::BAD_AUDIO, e))?
    .map_err(|e| CommandError::logged(CommandError::BAD_AUDIO, e))?;
    let warnings = quality::check_quality(&report)?;
    if cancel.is_cancelled() {
        return Err(CommandError::new(CommandError::CANCELLED));
    }

    // Окна для эмбеддингов голоса и нейросегментация просятся, только если их
    // использует конвейер.
    let options = PipelineOptions::default();
    let (window_sec, step_sec) = options
        .engine_options()
        .windows
        .map_or((0.0, 0.0), |w| (w.window_sec(), w.step_sec()));
    let neural = options.neural_segmentation;
    let model_overlaps = options.needs_model_overlaps();
    // Горячие слова: имена из голосов участников и список из настроек.
    let store_for_words = store.clone();
    let hotwords = tauri::async_runtime::spawn_blocking(move || {
        hotwords_line(&merge_hotwords(
            &crate::voices::names(&store_for_words),
            &crate::settings::load(&store_for_words).hotwords,
        ))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
    let forward = on_progress.clone();
    let summary: AnalyzeSummary = speech
        .analyze(AnalyzeRequest {
            wav_path: wav.to_string_lossy().into_owned(),
            out_path: segments_file.to_string_lossy().into_owned(),
            window_sec,
            step_sec,
            spans_path: neural.then(|| spans_file.to_string_lossy().into_owned()),
            neural_block_sec: if neural { NEURAL_BLOCK_SEC } else { 0.0 },
            overlaps_path: model_overlaps.then(|| overlaps_file.to_string_lossy().into_owned()),
            overlap_threshold: options.model_overlap_threshold,
            min_overlap_sec: MIN_SILENT_OVERLAP_SEC,
            hotwords,
            on_progress: Channel::new(move |body| {
                if let InvokeResponseBody::Json(text) = body
                    && let Ok(p) = serde_json::from_str::<AnalyzeProgress>(&text)
                {
                    let _ = forward.send(Progress {
                        phase: Phase::of_engine(p.phase.as_deref()),
                        done_sec: p.done_sec,
                        total_sec: p.total_sec,
                    });
                }
                Ok(())
            }),
        })
        .await
        .map_err(|e| match e.rejected_code() {
            Some("cancelled") => CommandError::new(CommandError::CANCELLED),
            Some(code) => CommandError::logged(code, &e),
            None => CommandError::logged(CommandError::ENGINE_FAILED, &e),
        })?;

    let _ = on_progress.send(Progress {
        phase: Phase::Voices,
        done_sec: summary.duration_sec,
        total_sec: summary.duration_sec,
    });
    // Порог слияния групп подобран под модель голоса, поэтому приходит с устройства вместе
    // с ней; образцы и центры берутся только этой модели.
    let options = PipelineOptions {
        same_speaker_cosine: same_speaker::checked_same_speaker(summary.same_speaker_cosine)
            .ok_or_else(|| {
                CommandError::logged(
                    CommandError::ENGINE_FAILED,
                    format!("порог слияния спикеров {}", summary.same_speaker_cosine),
                )
            })?,
        ..options
    };
    let summary_voice_model = summary.embedding_model.clone();
    let created_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let meta = TranscriptMeta {
        created_at: rfc3339_utc(created_secs),
        audio: AudioInfo {
            file: display_name(name.as_deref(), &input),
            duration_sec: report.duration_sec,
            sample_rate: TARGET_SAMPLE_RATE,
        },
        engine: EngineInfo {
            vad: summary.vad_model,
            asr: summary.asr_models,
            embedding: summary.embedding_model,
        },
    };
    let source = input.clone();
    let store_for_build = store.clone();
    let cancel_for_build = cancel.clone();
    let (mut transcript, centers) = tauri::async_runtime::spawn_blocking(move || {
        let (store, cancel) = (store_for_build, cancel_for_build);
        let segments = read_segments(&segments_file)
            .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))?;
        let spans = if options.neural_segmentation {
            read_spans(&spans_file)
                .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))?
        } else {
            Vec::new()
        };
        let overlaps = if options.needs_model_overlaps() {
            read_overlaps(&overlaps_file)
                .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))?
        } else {
            ModelOverlaps::default()
        };
        let samples = crate::voices::samples(&store, &summary_voice_model);
        let segmentation = Segmentation {
            spans: &spans,
            overlaps: &overlaps,
        };
        let (transcript, centers) = build_transcript_from(
            segments,
            segmentation,
            &samples,
            count,
            &options,
            meta,
            &cancel,
        )
        .map_err(|e| match e {
            PipelineError::Cancelled => CommandError::new(CommandError::CANCELLED),
            PipelineError::NoSpeech => CommandError::new(CommandError::NO_SPEECH),
        })?;
        Ok::<_, CommandError>((transcript, centers))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))??;
    // Ребёнок или взрослый: без модели или при её сбое расшифровка остаётся без меток.
    if let Some(request) = speaker_ages::age_request(&wav, &transcript) {
        match speech.speaker_ages(request).await {
            Ok(ages) => speaker_ages::apply_ages(&mut transcript, &ages),
            Err(e) if e.rejected_code() == Some("cancelled") => {
                return Err(CommandError::new(CommandError::CANCELLED));
            }
            Err(e) => log::warn!("ребёнок или взрослый не определён: {e}"),
        }
    }
    // Эмоции реплик: без модели или при её сбое расшифровка остаётся без эмоций.
    if let Some(request) = turn_emotions::emotion_request(&wav, &transcript) {
        match speech.turn_emotions(request).await {
            Ok(emotions) => turn_emotions::apply_emotions(&mut transcript, &emotions),
            Err(e) if e.rejected_code() == Some("cancelled") => {
                return Err(CommandError::new(CommandError::CANCELLED));
            }
            Err(e) => log::warn!("эмоции реплик не определены: {e}"),
        }
    }
    if cancel.is_cancelled() {
        return Err(CommandError::new(CommandError::CANCELLED));
    }
    let transcript = tauri::async_runtime::spawn_blocking(move || {
        save_meeting(
            &store,
            NewMeeting {
                transcript,
                centers: &centers,
                audio: &source,
                created_secs,
            },
        )
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))??;
    log::info!(
        "расшифровка: спикеров {}, ходов {}",
        transcript.speakers.len(),
        transcript.turns.len()
    );
    drop(slot);
    Ok(TranscribeResult {
        transcript,
        warnings,
    })
}

/// Имя исходного файла для расшифровки: имя, присланное интерфейсом (у импорта это
/// имя выбранного файла), иначе имя файла на диске. Только последний компонент пути,
/// без управляющих символов, до 200 знаков.
fn display_name(given: Option<&str>, input: &Path) -> String {
    let from_disk = input
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let raw = given.unwrap_or(&from_disk);
    let last = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let clean: String = last.chars().filter(|c| !c.is_control()).take(200).collect();
    if clean.trim().is_empty() {
        from_disk
    } else {
        clean
    }
}

/// Отмена идущей обработки. Если ничего не идёт, ничего и не происходит.
#[tauri::command]
pub async fn cancel_transcription<R: Runtime>(
    app: AppHandle<R>,
    running: State<'_, Running>,
) -> Result<(), CommandError> {
    if running.cancel() {
        app.speech()
            .cancel_analysis()
            .await
            .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::display_name;
    use std::path::Path;

    #[test]
    fn display_name_is_the_last_component_without_control_chars() {
        let disk = Path::new("/data/recordings/imp-1.wav");
        assert_eq!(display_name(None, disk), "imp-1.wav");
        assert_eq!(display_name(Some("встреча.m4a"), disk), "встреча.m4a");
        assert_eq!(display_name(Some("../../etc/passwd"), disk), "passwd");
        assert_eq!(display_name(Some("a\\b\u{0}c.mp3"), disk), "bc.mp3");
        assert_eq!(display_name(Some("  "), disk), "imp-1.wav");
        assert_eq!(
            display_name(Some(&"я".repeat(300)), disk).chars().count(),
            200
        );
    }
}
