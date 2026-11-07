//! Открытие встречи из списка: расшифровка и своё название.

use tauri::{AppHandle, Runtime};

use super::opened_meeting::OpenedMeeting;
use crate::transcription::CommandError;
use crate::vault::FilesStore;

/// Если индекс не читается, встреча всё равно открывается, с названием по умолчанию.
pub fn open(store: &FilesStore, meeting: &str) -> Result<OpenedMeeting, CommandError> {
    let (id, transcript) = super::load(store, meeting)?;
    let title = super::title(store, &id);
    let has_audio = super::audio_file::path(store.dir(), &id).is_some();
    Ok(OpenedMeeting {
        transcript,
        title,
        has_audio,
    })
}

#[tauri::command]
pub async fn open_meeting<R: Runtime>(
    app: AppHandle<R>,
    meeting: String,
) -> Result<OpenedMeeting, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || open(&store, &meeting))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::meeting_dir::{AUDIO, MeetingDir};
    use super::super::{NO_TRANSCRIPT, index};
    use super::*;

    #[test]
    fn meeting_opens_with_its_title_and_unknown_one_is_no_transcript() {
        let store = crate::vault::test_store("open-meeting");
        let t = super::super::test_transcript("2026-11-07T10:00:00Z");
        super::super::test_meeting(&store, &t);
        let opened = open(&store, "2026-11-07T10:00:00Z").unwrap();
        assert_eq!(opened.transcript, t);
        assert_eq!(opened.title, None);
        assert!(!opened.has_audio);
        index::update(&store, |m| {
            m[0].title = Some("Планёрка".into());
            Ok(())
        })
        .unwrap();
        let opened = open(&store, "2026-11-07T10:00:00Z").unwrap();
        assert_eq!(opened.title.as_deref(), Some("Планёрка"));
        for bad in ["2026-11-07T10:00:01Z", "../meetings.json"] {
            assert_eq!(open(&store, bad).unwrap_err().code, NO_TRANSCRIPT);
        }
    }

    #[test]
    fn meeting_without_audio_opens_with_its_text_and_says_so() {
        let store = crate::vault::test_store("open-meeting-audio");
        let t = super::super::test_transcript("2026-11-07T10:00:00Z");
        let id = super::super::test_meeting(&store, &t);
        let audio = MeetingDir::of(&id).path(store.dir()).join(AUDIO);
        std::fs::write(&audio, b"RIFF").unwrap();
        assert!(open(&store, id.as_str()).unwrap().has_audio);
        super::super::remove_audio::remove_audio(&store, std::slice::from_ref(&id));
        let opened = open(&store, id.as_str()).unwrap();
        assert!(!opened.has_audio);
        assert_eq!(opened.transcript, t);
    }
}
