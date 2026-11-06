//! Протокол встречи языковой моделью на устройстве: краткое содержание, решения и задачи
//! по последней расшифровке. Модель запускает Kotlin-плагин, запрос и хранение результата
//! на стороне ядра.

mod answer;
pub mod last_summary;
mod prompt;

use nixyon_android::{SpeechExt, SummaryProgress, SummaryRequest};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime, State};

use crate::transcription::{CommandError, Running, last_transcript};

pub(crate) const NO_TRANSCRIPT: &str = "no_transcript";

/// Работа языковой модели: протокол или подсказка имён, одно за раз, отдельно от
/// расшифровки. Модель на устройстве одна, отмена у обоих общая (`cancel_summary`).
#[derive(Default)]
pub struct SummaryRunning(pub(crate) Running);

/// Ошибка плагина языковой модели как ошибка команды: отмена отдельно, коды плагина как есть.
pub(crate) fn llm_error(e: nixyon_android::Error) -> CommandError {
    match e.rejected_code() {
        Some("llm_cancelled") => CommandError::new(CommandError::CANCELLED),
        Some(code) => CommandError::logged(code, &e),
        None => CommandError::logged("llm_failed", &e),
    }
}

/// Есть ли на устройстве языковая модель. Без неё кнопки протокола нет.
#[tauri::command]
pub async fn summary_available<R: Runtime>(app: AppHandle<R>) -> bool {
    match app.speech().summary_status().await {
        Ok(status) => status.available,
        Err(e) => {
            log::info!("языковой модели нет: {e}");
            false
        }
    }
}

/// Протокол последней расшифровки. Ход работы идёт в канал.
#[tauri::command]
pub async fn summarize_meeting<R: Runtime>(
    app: AppHandle<R>,
    running: State<'_, SummaryRunning>,
    on_progress: Channel<SummaryProgress>,
) -> Result<String, CommandError> {
    let slot = running
        .0
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;
    let store = crate::vault::storage(&app).await?.store;
    let store_for_load = store.clone();
    let (transcript, user) = tauri::async_runtime::spawn_blocking(move || {
        let transcript = last_transcript::load(&store_for_load)
            .ok_or_else(|| CommandError::new(NO_TRANSCRIPT))?;
        let user = prompt::meeting_text(&transcript)
            .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))?;
        Ok::<_, CommandError>((transcript, user))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))??;
    if slot.cancel.is_cancelled() {
        return Err(CommandError::new(CommandError::CANCELLED));
    }

    let forward = on_progress.clone();
    let reply = app
        .speech()
        .summarize(SummaryRequest {
            system: prompt::SYSTEM.to_owned(),
            user,
            max_tokens: prompt::MAX_TOKENS,
            on_progress: Channel::new(move |body| {
                if let InvokeResponseBody::Json(text) = body
                    && let Ok(p) = serde_json::from_str::<SummaryProgress>(&text)
                {
                    let _ = forward.send(p);
                }
                Ok(())
            }),
        })
        .await
        .map_err(llm_error)?;
    // Отмена могла прийти раньше, чем Kotlin начал работу, и не дойти до модели.
    if slot.cancel.is_cancelled() {
        return Err(CommandError::new(CommandError::CANCELLED));
    }
    let text = answer::clean_answer(&reply.text).ok_or_else(|| {
        log::error!("llm_failed: пустой ответ модели");
        CommandError::new("llm_failed")
    })?;

    let saved = text.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Расшифровку могли заменить, пока модель работала: протокол чужой расшифровке
        // не сохраняется.
        let current = last_transcript::load(&store);
        if current.as_ref().map(|t| &t.created_at) != Some(&transcript.created_at) {
            return Err(CommandError::new(NO_TRANSCRIPT));
        }
        last_summary::save(&store, &transcript, &saved)
            .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))??;
    drop(slot);
    Ok(text)
}

/// Отмена составления протокола. Если ничего не идёт, ничего и не происходит.
#[tauri::command]
pub async fn cancel_summary<R: Runtime>(
    app: AppHandle<R>,
    running: State<'_, SummaryRunning>,
) -> Result<(), CommandError> {
    if running.0.cancel() {
        app.speech()
            .cancel_summary()
            .await
            .map_err(|e| CommandError::logged("llm_failed", e))?;
    }
    Ok(())
}

/// Протокол последней расшифровки, если он уже составлен.
#[tauri::command]
pub async fn last_summary<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || {
        last_transcript::load(&store).and_then(|t| last_summary::load(&store, &t))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
}
