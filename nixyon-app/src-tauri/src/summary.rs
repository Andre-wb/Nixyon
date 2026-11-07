//! Протокол встречи языковой моделью на устройстве: краткое содержание, решения и задачи
//! по расшифровке встречи. Модель запускает Kotlin-плагин, запрос и хранение результата
//! на стороне ядра.

pub(crate) mod answer;
mod prompt;

use nixyon_android::{SpeechExt, SummaryProgress, SummaryRequest};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime, State};

use crate::meetings::{MeetingId, edit_lock, summary_file};
use crate::transcription::{CommandError, Running};
use crate::vault::FilesStore;

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

/// Протокол встречи. Ход работы идёт в канал.
#[tauri::command]
pub async fn summarize_meeting<R: Runtime>(
    app: AppHandle<R>,
    running: State<'_, SummaryRunning>,
    meeting: String,
    on_progress: Channel<SummaryProgress>,
) -> Result<String, CommandError> {
    let slot = running
        .0
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;
    let store = crate::vault::storage(&app).await?.store;
    let store_for_load = store.clone();
    let (id, user, language) = tauri::async_runtime::spawn_blocking(move || {
        let (id, transcript) = crate::meetings::load(&store_for_load, &meeting)?;
        let user = prompt::meeting_text(&transcript)
            .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))?;
        Ok::<_, CommandError>((id, user, transcript.language))
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
            system: prompt::system(language).to_owned(),
            user,
            max_tokens: prompt::MAX_TOKENS,
            language,
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
    tauri::async_runtime::spawn_blocking(move || save_if_exists(&store, &id, &saved))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))??;
    drop(slot);
    Ok(text)
}

/// Встречи могло не стать, пока модель работала: её папку протокол не воскрешает.
/// Проверка и запись идут под блокировкой правок, чтобы удаление не прошло между ними.
fn save_if_exists(store: &FilesStore, id: &MeetingId, text: &str) -> Result<(), CommandError> {
    edit_lock::locked(|| {
        if !crate::meetings::exists(store, id) {
            return Err(CommandError::new(crate::meetings::NO_TRANSCRIPT));
        }
        summary_file::save(store, id, text)
            .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
    })
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

/// Протокол встречи, если он уже составлен.
#[tauri::command]
pub async fn meeting_summary<R: Runtime>(
    app: AppHandle<R>,
    meeting: String,
) -> Result<Option<String>, CommandError> {
    let id = crate::meetings::meeting_id(&meeting)?;
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || summary_file::load(&store, &id))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "2026-11-07T10:00:00Z";

    #[test]
    fn summary_is_saved_to_an_existing_meeting() {
        let store = crate::vault::test_store("summary-save");
        let id = crate::meetings::test_meeting(&store, &crate::meetings::test_transcript(ID));
        save_if_exists(&store, &id, "Итоги").unwrap();
        assert_eq!(summary_file::load(&store, &id).as_deref(), Some("Итоги"));
    }

    #[test]
    fn summary_finished_after_deletion_does_not_bring_the_meeting_folder_back() {
        let store = crate::vault::test_store("summary-after-delete");
        let id = crate::meetings::test_meeting(&store, &crate::meetings::test_transcript(ID));
        crate::meetings::delete_meeting::delete(&store, ID, &|d| std::fs::remove_dir_all(d))
            .unwrap();
        assert_eq!(
            save_if_exists(&store, &id, "Итоги").unwrap_err().code,
            crate::meetings::NO_TRANSCRIPT
        );
        assert!(!crate::meetings::exists(&store, &id));
        let left = std::fs::read_dir(store.dir().join("meetings"))
            .unwrap()
            .count();
        assert_eq!(left, 0);
    }
}
