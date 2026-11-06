//! Проигрывание исходной записи последней расшифровки. Звук играет Kotlin-плагин
//! (MediaPlayer), здесь выбор файла, проверка позиции и пересылка событий.

use std::path::PathBuf;

use nixyon_android::{PlayRequest, PlayerEvent, PlayerOpenRequest, SpeechExt};
use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime};

use crate::media::plugin_error;
use crate::transcription::allowed_path::resolve_input;
use crate::transcription::{CommandError, last_audio};

const NO_AUDIO: &str = "no_audio";
const BAD_POSITION: &str = "bad_position";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackOpened {
    pub duration_sec: f64,
}

/// Позиция для перехода: число секунд не меньше нуля; None: с текущего места.
fn checked_position(from_sec: Option<f64>) -> Result<Option<f64>, CommandError> {
    match from_sec {
        Some(sec) if !(sec.is_finite() && sec >= 0.0) => Err(CommandError::new(BAD_POSITION)),
        other => Ok(other),
    }
}

/// Открывает запись последней расшифровки. Если записи уже нет (её заменила новая
/// запись или импорт), ошибка `no_audio`.
#[tauri::command]
pub async fn open_playback<R: Runtime>(
    app: AppHandle<R>,
    on_event: Channel<PlayerEvent>,
) -> Result<PlaybackOpened, CommandError> {
    let speech = app.speech();
    let dirs = crate::vault::storage(&app).await?.paths;
    let roots: Vec<PathBuf> = [
        Some(&dirs.files),
        Some(&dirs.cache),
        dirs.external_files.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .collect();
    let files = PathBuf::from(&dirs.files);
    let path = tauri::async_runtime::spawn_blocking(move || {
        last_audio::load(&files).and_then(|p| resolve_input(&p, &roots))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
    .ok_or_else(|| CommandError::new(NO_AUDIO))?;
    let opened = speech
        .player_open(PlayerOpenRequest {
            path: path.to_string_lossy().into_owned(),
            on_event: Channel::new(move |body| {
                if let InvokeResponseBody::Json(text) = body
                    && let Ok(event) = serde_json::from_str::<PlayerEvent>(&text)
                {
                    let _ = on_event.send(event);
                }
                Ok(())
            }),
        })
        .await
        .map_err(plugin_error)?;
    Ok(PlaybackOpened {
        duration_sec: opened.duration_sec,
    })
}

#[tauri::command]
pub async fn play_audio<R: Runtime>(
    app: AppHandle<R>,
    from_sec: Option<f64>,
) -> Result<(), CommandError> {
    let from_sec = checked_position(from_sec)?;
    app.speech()
        .player_play(PlayRequest { from_sec })
        .await
        .map_err(plugin_error)
}

#[tauri::command]
pub async fn pause_audio<R: Runtime>(app: AppHandle<R>) -> Result<(), CommandError> {
    app.speech().player_pause().await.map_err(plugin_error)
}

#[tauri::command]
pub async fn close_playback<R: Runtime>(app: AppHandle<R>) -> Result<(), CommandError> {
    app.speech().player_close().await.map_err(plugin_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_finite_non_negative_positions_are_accepted() {
        assert_eq!(checked_position(None).unwrap(), None);
        assert_eq!(checked_position(Some(0.0)).unwrap(), Some(0.0));
        assert_eq!(checked_position(Some(12.5)).unwrap(), Some(12.5));
        for bad in [-0.1, f64::NAN, f64::INFINITY] {
            assert_eq!(checked_position(Some(bad)).unwrap_err().code, BAD_POSITION);
        }
    }
}
