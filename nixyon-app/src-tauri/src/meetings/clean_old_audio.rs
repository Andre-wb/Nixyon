//! Автоочистка: звук встреч старше выбранного в настройках срока удаляется, текст
//! остаётся. Запускает интерфейс при старте и после смены настройки: только он знает пояс
//! устройства.

use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Runtime};

use super::audio_cutoff::{cutoff, older_than, valid_offset};
use super::index;
use super::remove_audio::remove_audio;
use crate::transcription::CommandError;
use crate::vault::FilesStore;

pub const BAD_TIME_ZONE: &str = "bad_time_zone";

/// Сколько встреч осталось без звука. `days`: None, очистка выключена.
pub fn clean(
    store: &FilesStore,
    days: Option<u32>,
    now_secs: u64,
    offset_minutes: i32,
) -> Result<usize, CommandError> {
    if !valid_offset(offset_minutes) {
        return Err(CommandError::new(BAD_TIME_ZONE));
    }
    let Some(days) = days else {
        return Ok(0);
    };
    let meetings =
        index::load(store).map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
    let border = cutoff(now_secs, offset_minutes, days);
    let old = older_than(meetings.iter().map(|m| &m.id), &border);
    Ok(remove_audio(store, &old))
}

#[tauri::command]
pub async fn clean_old_audio<R: Runtime>(
    app: AppHandle<R>,
    offset_minutes: i32,
) -> Result<usize, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    tauri::async_runtime::spawn_blocking(move || {
        let days = crate::settings::load(&store).audio_cleanup_days;
        clean(&store, days, now, offset_minutes)
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::audio_file;
    use super::super::meeting_dir::{AUDIO, MeetingDir};
    use super::super::meeting_id::MeetingId;
    use super::super::{test_meeting, test_transcript, transcript_file};
    use super::*;

    // 2026-11-07T10:00:00Z, в Москве 13:00.
    const NOW: u64 = 1_794_045_600;
    const MOSCOW: i32 = 180;

    fn with_audio(store: &FilesStore, created_at: &str) -> MeetingId {
        let id = test_meeting(store, &test_transcript(created_at));
        std::fs::write(MeetingDir::of(&id).path(store.dir()).join(AUDIO), b"RIFF").unwrap();
        id
    }

    #[test]
    fn audio_older_than_n_local_days_goes_and_text_stays() {
        let store = crate::vault::test_store("clean-old-audio");
        assert_eq!(
            MeetingId::from_unix_secs(NOW).as_str(),
            "2026-11-07T10:00:00Z"
        );
        // Местная полночь 31 октября: 30 октября 21:00 по UTC.
        let old = with_audio(&store, "2026-10-30T20:59:59Z");
        let border = with_audio(&store, "2026-10-30T21:00:00Z");
        let fresh = with_audio(&store, "2026-11-07T09:00:00Z");
        assert_eq!(clean(&store, Some(7), NOW, MOSCOW).unwrap(), 1);
        assert_eq!(audio_file::path(store.dir(), &old), None);
        assert!(transcript_file::load(&store, &old).is_some());
        assert!(audio_file::path(store.dir(), &border).is_some());
        assert!(audio_file::path(store.dir(), &fresh).is_some());
    }

    #[test]
    fn switched_off_cleanup_removes_nothing() {
        let store = crate::vault::test_store("clean-old-audio-off");
        let old = with_audio(&store, "2020-01-01T00:00:00Z");
        assert_eq!(clean(&store, None, NOW, MOSCOW).unwrap(), 0);
        assert!(audio_file::path(store.dir(), &old).is_some());
    }

    #[test]
    fn impossible_time_zone_is_rejected_before_anything_is_removed() {
        let store = crate::vault::test_store("clean-old-audio-zone");
        let old = with_audio(&store, "2020-01-01T00:00:00Z");
        let err = clean(&store, Some(7), NOW, 24 * 60).unwrap_err();
        assert_eq!(err.code, BAD_TIME_ZONE);
        assert!(audio_file::path(store.dir(), &old).is_some());
    }

    #[test]
    fn broken_index_is_an_error_and_removes_nothing() {
        let store = crate::vault::test_store("clean-old-audio-broken");
        let old = with_audio(&store, "2020-01-01T00:00:00Z");
        store.write(crate::meetings::INDEX_FILE, b"{").unwrap();
        let err = clean(&store, Some(7), NOW, MOSCOW).unwrap_err();
        assert_eq!(err.code, CommandError::STORAGE_FAILED);
        assert!(audio_file::path(store.dir(), &old).is_some());
    }
}
