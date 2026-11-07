//! Повторная обработка одной реплики: её звук вырезается из записи и распознаётся
//! заново, затем языковая модель проверяет только этот текст. Расшифровка при этом не
//! меняется — итог возвращает интерфейс и заменяет текст обычной правкой реплики,
//! поэтому лимит модели тратится на фразу, а не на всю встречу.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use nixyon_android::{
    AnalyzeProgress, AnalyzeRequest, SpeechExt, SummaryProgress, SummaryRequest, read_segments,
};
use nixyon_audio::{AudioError, slice_reader_to_wav16k};
use nixyon_core::{SegmentResult, hotwords_line};
use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime, State};

use crate::settings::merge_hotwords;
use crate::summary::{NO_TRANSCRIPT, SummaryRunning, answer};
use crate::transcription::allowed_path::resolve_input;
use crate::transcription::{
    CommandError, Running, Slot, last_audio, last_transcript, temp_files::TempFiles,
};

const NO_AUDIO: &str = "no_audio";
const UNKNOWN_TURN: &str = "unknown_turn";

/// Фраза длинее этого в проверку не идёт: запрос и ответ модели должны оставаться
/// маленькими, иначе проверка одной фразы стоила бы как протокол встречи.
const MAX_CHECK_CHARS: usize = 800;
/// Предел длины ответа модели: хватает на фразу в `MAX_CHECK_CHARS` символов.
const CHECK_MAX_TOKENS: u32 = 512;

/// Инструкция модели: сверить записанный текст с повторным распознаванием и вернуть
/// один итоговый текст фразы. Инструкция русская при любом языке интерфейса, как и
/// инструкция протокола встречи.
const SYSTEM: &str = "Ты сверяешь текст одной фразы с её повторным распознаванием по звуку. \
Даны два варианта: «Расшифровка» — как фраза записана сейчас, «Распознано» — как её \
услышал распознаватель заново. Брать исправления из «Распознано» можно только там, где \
видно ошибку распознавания или опечатку; сомнительные различия оставляй как в расшифровке. \
Сохраняй язык, смысл и знаки препинания. Ответь одной строкой — итоговым текстом фразы, \
без пояснений, кавычек и номеров. Если ошибок нет, повтори текст расшифровки.";

/// Стадия повторной обработки.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Фрагмент режется из записи и распознаётся заново.
    Listen,
    /// Распознанный текст проверяет языковая модель.
    Check,
}

/// Ход повторной обработки: стадия и сколько сделано из скольких. На стадии `listen`
/// это секунды фрагмента, на стадии `check` — токены.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReprocessProgress {
    pub stage: Stage,
    pub done: f64,
    pub total: f64,
}

/// Итог повторной обработки. Ничего не сохраняется: применять или нет — решает
/// интерфейс, замена идёт обычной правкой реплики.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReprocessResult {
    /// Текст фрагмента после повторного распознавания.
    pub recognized: String,
    /// Исправление языковой модели; None: модель фразу не проверяла.
    pub correction: Option<String>,
    /// true, если фразу проверяла языковая модель.
    pub checked: bool,
    /// true, если итоговый текст отличается от текущего: есть что заменять.
    pub changed: bool,
}

/// Повторная обработка реплики `id`. Ход обеих стадий идёт в канал; отмена — общей
/// отменой распознавания и языковой модели. Оба слота заняются сразу: пока фраза
/// обрабатывается, не начнётся ни новая расшифровка, ни протокол с подсказкой имён.
#[tauri::command]
pub async fn reprocess_turn<R: Runtime>(
    app: AppHandle<R>,
    listening: State<'_, Running>,
    checking: State<'_, SummaryRunning>,
    id: u32,
    on_progress: Channel<ReprocessProgress>,
) -> Result<ReprocessResult, CommandError> {
    let listen_slot = listening
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;
    let check_slot = checking
        .0
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;

    let storage = crate::vault::storage(&app).await?;
    let (store, dirs) = (storage.store, storage.paths);
    let roots: Vec<PathBuf> = [
        Some(&dirs.files),
        Some(&dirs.cache),
        dirs.external_files.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .collect();
    let files_dir = PathBuf::from(&dirs.files);
    let cache_dir = PathBuf::from(&dirs.cache);
    let store_for_load = store.clone();
    let (turn, audio) = tauri::async_runtime::spawn_blocking(move || {
        let transcript = last_transcript::load(&store_for_load)
            .ok_or_else(|| CommandError::new(NO_TRANSCRIPT))?;
        let turn = transcript
            .turns
            .into_iter()
            .find(|t| t.id == id)
            .ok_or_else(|| CommandError::new(UNKNOWN_TURN))?;
        let audio = last_audio::load(&files_dir)
            .and_then(|p| resolve_input(&p, &roots))
            .ok_or_else(|| CommandError::new(NO_AUDIO))?;
        Ok::<_, CommandError>((turn, audio))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))??;
    let duration = (turn.end - turn.start).max(0.0);
    let _ = on_progress.send(ReprocessProgress {
        stage: Stage::Listen,
        done: 0.0,
        total: duration,
    });

    let store_for_words = store.clone();
    let hotwords = tauri::async_runtime::spawn_blocking(move || {
        hotwords_line(&merge_hotwords(
            &crate::voices::names(&store_for_words),
            &crate::settings::load(&store_for_words).hotwords,
        ))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let wav = cache_dir.join(format!("nixyon-{stamp}.wav"));
    let segments_file = cache_dir.join(format!("nixyon-{stamp}.segments.json"));
    let _temp = TempFiles::new(vec![wav.clone(), segments_file.clone()]);

    // Нарезка фрагмента: тот же конвейер, что и для всей записи, только уровни
    // меряются по самому фрагменту.
    let key = std::sync::Arc::clone(store.key());
    let (input_for_slice, wav_for_slice) = (audio.clone(), wav.clone());
    let (from, to) = (turn.start, turn.end);
    tauri::async_runtime::spawn_blocking(move || {
        let source = crate::vault::open_read(&input_for_slice, &key)?;
        slice_reader_to_wav16k(source, &wav_for_slice, from, to)
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::BAD_AUDIO, e))?
    .map_err(|e| match e {
        AudioError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("запись для повторной обработки не найдена");
            CommandError::new(NO_AUDIO)
        }
        other => CommandError::logged(CommandError::BAD_AUDIO, other),
    })?;
    if cancelled(&listen_slot, &check_slot) {
        return Err(CommandError::new(CommandError::CANCELLED));
    }

    // Окна голоса и нейросегментация не нужны: из фрагмента нужен только текст.
    let forward = on_progress.clone();
    app.speech()
        .analyze(AnalyzeRequest {
            wav_path: wav.to_string_lossy().into_owned(),
            out_path: segments_file.to_string_lossy().into_owned(),
            window_sec: 0.0,
            step_sec: 0.0,
            spans_path: None,
            neural_block_sec: 0.0,
            hotwords,
            on_progress: Channel::new(move |body| {
                if let InvokeResponseBody::Json(text) = body
                    && let Ok(p) = serde_json::from_str::<AnalyzeProgress>(&text)
                {
                    let _ = forward.send(ReprocessProgress {
                        stage: Stage::Listen,
                        done: p.done_sec,
                        total: p.total_sec,
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
    if cancelled(&listen_slot, &check_slot) {
        return Err(CommandError::new(CommandError::CANCELLED));
    }

    let recognized = tauri::async_runtime::spawn_blocking(move || {
        let segments = read_segments(&segments_file)
            .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))?;
        Ok::<_, CommandError>(joined_text(segments))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))??;
    if recognized.is_empty() {
        return Err(CommandError::new(CommandError::NO_SPEECH));
    }

    // Языковая модель проверяет только эту фразу и только если она есть на устройстве;
    // без неё или при слишком длинной фразе остаётся результат повторного
    // распознавания, и лимит модели не тратится вовсе.
    let correction = if let Some(user) = check_prompt(&app, &turn.text, &recognized).await {
        if check_slot.cancel.is_cancelled() {
            return Err(CommandError::new(CommandError::CANCELLED));
        }
        let forward = on_progress.clone();
        match app
            .speech()
            .summarize(SummaryRequest {
                system: SYSTEM.to_owned(),
                user,
                max_tokens: CHECK_MAX_TOKENS,
                on_progress: Channel::new(move |body| {
                    if let InvokeResponseBody::Json(text) = body
                        && let Ok(p) = serde_json::from_str::<SummaryProgress>(&text)
                    {
                        let _ = forward.send(ReprocessProgress {
                            stage: Stage::Check,
                            done: f64::from(p.done),
                            total: f64::from(p.total),
                        });
                    }
                    Ok(())
                }),
            })
            .await
        {
            Ok(reply) => {
                if check_slot.cancel.is_cancelled() {
                    return Err(CommandError::new(CommandError::CANCELLED));
                }
                correction_from(&reply.text)
            }
            // Проверка не состоялась, но фраза уже прослушана: итог возвращается без
            // исправления модели, повторять распознавание не зачем.
            Err(e) if e.rejected_code() == Some("llm_cancelled") => {
                return Err(CommandError::new(CommandError::CANCELLED));
            }
            Err(e) => {
                log::warn!("фраза не проверена моделью: {e}");
                None
            }
        }
    } else {
        None
    };
    drop(listen_slot);
    drop(check_slot);

    let candidate = correction.as_deref().unwrap_or(&recognized);
    Ok(ReprocessResult {
        checked: correction.is_some(),
        changed: candidate != turn.text,
        recognized,
        correction,
    })
}

/// Отмена пришла в любую из стадий.
fn cancelled(listen: &Slot<'_>, check: &Slot<'_>) -> bool {
    listen.cancel.is_cancelled() || check.cancel.is_cancelled()
}

/// Текст фрагмента из кусков распознавания: те же правила, что у текста хода в
/// конвейере — куски по времени, пустые пропускаются, между ними пробел.
fn joined_text(mut segments: Vec<SegmentResult>) -> String {
    segments.retain(|s| !s.text.trim().is_empty());
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    segments
        .iter()
        .map(|s| s.text.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Текст запроса к модели: оба варианта фразы целиком. None, если проверять нечего —
/// языковой модели нет на устройстве или фраза длиннее бюджета, тогда проверка
/// стоила бы дороже самой фразы и лимит тратится не на то.
async fn check_prompt<R: Runtime>(
    app: &AppHandle<R>,
    current: &str,
    recognized: &str,
) -> Option<String> {
    let available = match app.speech().summary_status().await {
        Ok(status) => status.available,
        Err(e) => {
            log::info!("фраза без проверки, языковой модели нет: {e}");
            return None;
        }
    };
    if !available {
        log::info!("фраза без проверки, языковой модели нет");
        return None;
    }
    if current.chars().count() > MAX_CHECK_CHARS || recognized.chars().count() > MAX_CHECK_CHARS {
        log::info!("фраза длиннее {MAX_CHECK_CHARS} знаков, проверка моделью пропущена");
        return None;
    }
    Some(prompt(current, recognized))
}

/// Оба варианта фразы целиком: модель сверяет их между собой.
fn prompt(current: &str, recognized: &str) -> String {
    format!("Расшифровка: {current}\nРаспознано: {recognized}")
}

/// Ответ модели — один текст фразы. Модель может обернуть его в кавычки или написать
/// несколько строк: всё сводится к одной строке. Пустой или подозрительно длинный
/// ответ (обрезанный или не про эту фразу) не берётся — заменять тогда нечем.
fn correction_from(raw: &str) -> Option<String> {
    let text = answer::clean_answer(raw)?;
    let joined: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean: String = joined.chars().filter(|c| !c.is_control()).collect();
    let text = unquote(clean.trim());
    if text.is_empty() || text.chars().count() > MAX_CHECK_CHARS {
        return None;
    }
    Some(text.to_owned())
}

/// Кавычки вокруг всего ответа — обёртка модели, а не часть фразы. Снимаются, только
/// если внутри нет такой же пары: иначе в кавычки завёрнута не вся фраза.
pub(crate) fn unquote(text: &str) -> &str {
    let mut out = text;
    for (open, close) in [('«', '»'), ('„', '“'), ('"', '"')] {
        if out.len() < open.len_utf8() + close.len_utf8()
            || !out.starts_with(open)
            || !out.ends_with(close)
        {
            continue;
        }
        let inner = &out[open.len_utf8()..out.len() - close.len_utf8()];
        if inner.contains(close) {
            continue;
        }
        out = inner.trim();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_text_joins_non_empty_segments_in_time_order() {
        let segment = |start: f64, text: &str| SegmentResult {
            start,
            end: start + 1.0,
            text: text.into(),
            tokens: Vec::new(),
            token_times: Vec::new(),
            embedding: Vec::new(),
            windows: Vec::new(),
        };
        assert_eq!(
            joined_text(vec![
                segment(2.0, " мир"),
                segment(0.0, "привет "),
                segment(4.0, "   "),
            ]),
            "привет мир"
        );
        assert_eq!(joined_text(Vec::new()), "");
    }

    #[test]
    fn model_answer_becomes_one_line_without_wrapper() {
        assert_eq!(
            correction_from("  Да, конечно.  ").as_deref(),
            Some("Да, конечно.")
        );
        assert_eq!(
            correction_from("Сначала\nсобрать отчёт.").as_deref(),
            Some("Сначала собрать отчёт.")
        );
        assert_eq!(
            correction_from("«Привет, мир!»").as_deref(),
            Some("Привет, мир!")
        );
        assert_eq!(
            correction_from("\"а\" и \"б\"").as_deref(),
            Some("\"а\" и \"б\"")
        );
        assert_eq!(correction_from("   \n  ").as_deref(), None);
    }

    #[test]
    fn empty_or_overlong_answer_is_not_taken() {
        assert_eq!(correction_from(""), None);
        assert_eq!(correction_from("«»"), None);
        let long = "слово ".repeat(MAX_CHECK_CHARS);
        assert_eq!(correction_from(&long), None);
        // Длинная, но в пределах бюджета — берётся.
        let fits: String = std::iter::repeat('а')
            .take(MAX_CHECK_CHARS)
            .collect();
        assert_eq!(correction_from(&fits).map(|s| s.chars().count()), Some(MAX_CHECK_CHARS));
    }

    #[test]
    fn prompt_shows_both_variants_of_the_phrase() {
        assert_eq!(prompt("текст", "тест"), "Расшифровка: текст\nРаспознано: тест");
    }
}
