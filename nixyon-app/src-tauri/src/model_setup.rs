//! Модели речи и языковая модель качаются с зеркала по выбору пользователя: в APK только
//! код и каталог моделей. Загрузку делает Kotlin-плагин, здесь команды для интерфейса.

mod pack_list;

use nixyon_android::{DownloadModelsRequest, ModelDownloadProgress, ModelsStatus, SpeechExt};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime};

use crate::transcription::CommandError;

const BAD_MODEL_PACK: &str = "bad_model_pack";
/// Заголовок уведомления приходит из словаря интерфейса; длиннее не бывает.
const MAX_TITLE_CHARS: usize = 200;

fn plugin_error(e: nixyon_android::Error) -> CommandError {
    match e.rejected_code() {
        Some(code @ (CommandError::CANCELLED | CommandError::BUSY)) => CommandError::new(code),
        Some(code) => CommandError::logged(code, &e),
        None => CommandError::logged("models_download_failed", &e),
    }
}

/// Какие модели есть в каталоге и какие уже скачаны.
#[tauri::command]
pub async fn models_status<R: Runtime>(app: AppHandle<R>) -> Result<ModelsStatus, CommandError> {
    app.speech().models_status().await.map_err(plugin_error)
}

/// Качает выбранные модели; обязательный набор речи плагин добавляет сам. После загрузки
/// модели речи сразу загружаются в фоне, как при старте приложения.
#[tauri::command]
pub async fn download_models<R: Runtime>(
    app: AppHandle<R>,
    packs: Vec<String>,
    allow_metered: bool,
    notification_title: String,
    on_progress: Channel<ModelDownloadProgress>,
) -> Result<(), CommandError> {
    let packs = pack_list::parse_packs(&packs).map_err(|_| CommandError::new(BAD_MODEL_PACK))?;
    let notification_title: String = notification_title.chars().take(MAX_TITLE_CHARS).collect();
    app.speech()
        .download_models(DownloadModelsRequest {
            packs,
            allow_metered,
            notification_title,
            on_progress: Channel::new(move |body| {
                if let InvokeResponseBody::Json(text) = body
                    && let Ok(p) = serde_json::from_str::<ModelDownloadProgress>(&text)
                {
                    let _ = on_progress.send(p);
                }
                Ok(())
            }),
        })
        .await
        .map_err(plugin_error)?;
    let handle = app.clone();
    tauri::async_runtime::spawn(async move { warm_up(&handle).await });
    Ok(())
}

#[tauri::command]
pub async fn cancel_model_download<R: Runtime>(app: AppHandle<R>) -> Result<(), CommandError> {
    app.speech()
        .cancel_model_download()
        .await
        .map_err(plugin_error)
}

/// false, только если плагин ответил, что набор речи не скачан. Без ответа (ноутбук, старый
/// плагин) движок пробуется как раньше и сам сообщит о причине.
pub async fn speech_ready<R: Runtime>(app: &AppHandle<R>) -> bool {
    match app.speech().models_status().await {
        Ok(status) => status
            .packs
            .iter()
            .any(|p| p.key == "speech" && p.installed),
        Err(_) => true,
    }
}

/// Загружает модели речи в фоне, чтобы к первой записи они были готовы.
pub async fn warm_up<R: Runtime>(app: &AppHandle<R>) {
    match app.speech().engine_status().await {
        Ok(status) => log::info!(
            "модели: {} ({}), загрузка {} мс, эмбеддинг {}",
            status.source,
            status.version,
            status.load_ms,
            status.embedding_dim
        ),
        Err(e) => log::error!("модели не загружены: {e}"),
    }
}
