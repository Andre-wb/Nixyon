//! Проигрывание исходной записи встречи. Звук играет Kotlin-плагин
//! (MediaPlayer), здесь выбор файла, проверка позиции и пересылка событий.

use std::path::PathBuf;

use nixyon_android::{
    PlayRequest, PlaybackRange, PlaybackRangesRequest, PlaybackSpeedRequest, PlayerEvent,
    PlayerOpenRequest, SkipRequest, SpeechExt,
};
use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime};

use crate::media::plugin_error;
use crate::meetings::audio_file;
use crate::transcription::CommandError;

const NO_AUDIO: &str = "no_audio";
const BAD_POSITION: &str = "bad_position";
const BAD_SPEED: &str = "bad_speed";
/// Крайние скорости, которые плеер принимает; интерфейс предлагает 0,75–2.
const SPEED_RANGE: std::ops::RangeInclusive<f64> = 0.25..=4.0;

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

/// Отрезки для проигрывания: все должны быть конечными, начинаться не ниже нуля и
/// кончаться позже начала.
fn checked_ranges(ranges: &[PlaybackRange]) -> Result<(), CommandError> {
    let valid = |r: &PlaybackRange| {
        r.start_sec.is_finite()
            && r.end_sec.is_finite()
            && r.start_sec >= 0.0
            && r.start_sec < r.end_sec
    };
    if ranges.iter().all(valid) {
        Ok(())
    } else {
        Err(CommandError::new(BAD_POSITION))
    }
}

/// Открывает запись встречи. Если у встречи записи нет, ошибка `no_audio`.
/// `title`: название для экрана блокировки.
#[tauri::command]
pub async fn open_playback<R: Runtime>(
    app: AppHandle<R>,
    meeting: String,
    on_event: Channel<PlayerEvent>,
    title: String,
) -> Result<PlaybackOpened, CommandError> {
    let id = crate::meetings::meeting_id(&meeting)?;
    let speech = app.speech();
    let dirs = crate::vault::storage(&app).await?.paths;
    let files = PathBuf::from(&dirs.files);
    let path = tauri::async_runtime::spawn_blocking(move || audio_file::path(&files, &id))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
        .ok_or_else(|| CommandError::new(NO_AUDIO))?;
    let opened = speech
        .player_open(PlayerOpenRequest {
            path: path.to_string_lossy().into_owned(),
            title,
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

/// Скорость проигрывания: 1 обычная, 0,75 медленнее, 2 вдвое быстрее.
#[tauri::command]
pub async fn set_playback_speed<R: Runtime>(
    app: AppHandle<R>,
    speed: f64,
) -> Result<(), CommandError> {
    if !SPEED_RANGE.contains(&speed) {
        return Err(CommandError::new(BAD_SPEED));
    }
    app.speech()
        .player_set_speed(PlaybackSpeedRequest { speed })
        .await
        .map_err(plugin_error)
}

/// Играть только эти отрезки подряд; при `None` играется вся запись. Между отрезками звук пропускается.
#[tauri::command]
pub async fn set_playback_ranges<R: Runtime>(
    app: AppHandle<R>,
    ranges: Option<Vec<PlaybackRange>>,
) -> Result<(), CommandError> {
    let limited = ranges.is_some();
    let ranges = ranges.unwrap_or_default();
    checked_ranges(&ranges)?;
    app.speech()
        .player_set_ranges(PlaybackRangesRequest { limited, ranges })
        .await
        .map_err(plugin_error)
}

/// Перемотка от текущего места на `delta_sec` секунд; при отрицательном значении назад.
#[tauri::command]
pub async fn skip_audio<R: Runtime>(app: AppHandle<R>, delta_sec: f64) -> Result<(), CommandError> {
    if !delta_sec.is_finite() {
        return Err(CommandError::new(BAD_POSITION));
    }
    app.speech()
        .player_skip(SkipRequest { delta_sec })
        .await
        .map_err(plugin_error)
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

    #[test]
    fn ranges_must_be_finite_and_run_forward() {
        let range = |start_sec, end_sec| PlaybackRange { start_sec, end_sec };
        assert!(checked_ranges(&[]).is_ok());
        assert!(checked_ranges(&[range(0.0, 1.0), range(2.0, 2.5)]).is_ok());
        for bad in [
            range(-1.0, 1.0),
            range(3.0, 3.0),
            range(4.0, 1.0),
            range(f64::NAN, 1.0),
        ] {
            assert_eq!(checked_ranges(&[bad]).unwrap_err().code, BAD_POSITION);
        }
    }

    #[test]
    fn speed_is_limited_to_the_player_range() {
        assert!(SPEED_RANGE.contains(&0.75));
        assert!(SPEED_RANGE.contains(&2.0));
        assert!(!SPEED_RANGE.contains(&0.0));
        assert!(!SPEED_RANGE.contains(&f64::NAN));
    }
}
