//! Источники звука: запись с микрофона и импорт файла. Сами запись и раскодирование
//! делает Kotlin-плагин; здесь команды для интерфейса и пересылка событий.

use nixyon_android::{
    ImportOutcome, ImportRequest, PauseRequest, RecordRequest, RecorderEvent, RecordingSaved,
    SpeechExt,
};
use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime};

use crate::transcription::CommandError;

/// Сколько файла уже раскодировано при импорте.
#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub done_sec: f64,
    pub total_sec: f64,
}

pub(crate) fn plugin_error(e: nixyon_android::Error) -> CommandError {
    match e.rejected_code() {
        Some(code @ ("mic_denied" | "cancelled" | "busy")) => CommandError::new(code),
        Some(code) => CommandError::logged(code, &e),
        None => CommandError::logged(CommandError::ENGINE_FAILED, &e),
    }
}

/// Начинает запись. В канал идут уровень громкости 10 раз в секунду, живой текст при
/// каждом его изменении (если в наборе есть живая модель языка речи из настроек) и событие остановки, если запись остановилась сама (предел, место, звонок, сворачивание без
/// foreground service). `notification_title` и `notification_channel`: тексты
/// уведомления о записи на языке интерфейса. `sample`: запись образца голоса, она не
/// заменяет запись последней встречи.
#[tauri::command]
pub async fn start_recording<R: Runtime>(
    app: AppHandle<R>,
    on_event: Channel<RecorderEvent>,
    notification_title: String,
    notification_channel: String,
    sample: Option<bool>,
) -> Result<u32, CommandError> {
    // Подготовка хранилища удаляет недописанные записи: она должна пройти до начала новой.
    let store = crate::vault::storage(&app).await?.store;
    // Язык речи из настроек; None: «Авто», живой текст сам перейдёт на английский.
    let language = tauri::async_runtime::spawn_blocking(move || {
        crate::settings::load(&store).speech_language.fixed()
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
    let started = app
        .speech()
        .start_recording(RecordRequest {
            language,
            notification_title,
            notification_channel,
            sample: sample.unwrap_or(false),
            on_event: Channel::new(move |body| {
                if let InvokeResponseBody::Json(text) = body
                    && let Ok(event) = serde_json::from_str::<RecorderEvent>(&text)
                {
                    let _ = on_event.send(event);
                }
                Ok(())
            }),
        })
        .await
        .map_err(plugin_error)?;
    Ok(started.sample_rate)
}

#[tauri::command]
pub async fn stop_recording<R: Runtime>(app: AppHandle<R>) -> Result<RecordingSaved, CommandError> {
    app.speech().stop_recording().await.map_err(plugin_error)
}

/// Ставит запись на паузу: микрофон открыт, звук не пишется. Продолжается тем же файлом.
/// Заголовок уведомления о записи меняется на переданный.
#[tauri::command]
pub async fn pause_recording<R: Runtime>(
    app: AppHandle<R>,
    notification_title: String,
) -> Result<(), CommandError> {
    app.speech()
        .pause_recording(PauseRequest { notification_title })
        .await
        .map_err(plugin_error)
}

#[tauri::command]
pub async fn resume_recording<R: Runtime>(
    app: AppHandle<R>,
    notification_title: String,
) -> Result<(), CommandError> {
    app.speech()
        .resume_recording(PauseRequest { notification_title })
        .await
        .map_err(plugin_error)
}

/// Запись с микрофона, оборванная падением приложения: её восстанавливает плагин, когда ядро
/// берёт ключ, до удаления недописанных файлов. Поэтому команда сначала ждёт подготовки
/// хранилища. Путь пустой, если восстанавливать нечего; запись отдаётся один раз.
#[tauri::command]
pub async fn recovered_recording<R: Runtime>(
    app: AppHandle<R>,
) -> Result<RecordingSaved, CommandError> {
    crate::vault::storage(&app).await?;
    app.speech()
        .recovered_recording()
        .await
        .map_err(plugin_error)
}

/// Открывает системный выбор аудиофайла и раскодирует выбранный в WAV.
#[tauri::command]
pub async fn import_audio<R: Runtime>(
    app: AppHandle<R>,
    on_progress: Channel<ImportProgress>,
) -> Result<ImportOutcome, CommandError> {
    crate::vault::storage(&app).await?;
    app.speech()
        .import_audio(ImportRequest {
            on_progress: Channel::new(move |body| {
                if let InvokeResponseBody::Json(text) = body
                    && let Ok(p) = serde_json::from_str::<ImportProgress>(&text)
                {
                    let _ = on_progress.send(p);
                }
                Ok(())
            }),
        })
        .await
        .map_err(plugin_error)
}

#[tauri::command]
pub async fn cancel_import<R: Runtime>(app: AppHandle<R>) -> Result<(), CommandError> {
    app.speech().cancel_import().await.map_err(plugin_error)
}

/// Открывает страницу приложения в настройках Android, чтобы включить микрофон.
#[tauri::command]
pub async fn open_app_settings<R: Runtime>(app: AppHandle<R>) -> Result<(), CommandError> {
    app.speech().open_app_settings().await.map_err(plugin_error)
}
