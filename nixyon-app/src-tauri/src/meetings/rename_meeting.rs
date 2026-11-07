//! Своё название встречи. Хранится только в индексе: JSON расшифровки не меняется. Само
//! название в лог не пишется, это личные данные.

use tauri::{AppHandle, Runtime};

use super::index;
use super::meeting_title::parse_title;
use super::{NO_TRANSCRIPT, meeting_id};
use crate::transcription::CommandError;
use crate::vault::FilesStore;

/// Новое название встречи; `None`: встреча снова называется датой и временем.
pub fn rename(
    store: &FilesStore,
    meeting: &str,
    title: &str,
) -> Result<Option<String>, CommandError> {
    let id = meeting_id(meeting)?;
    let title = parse_title(title).map_err(|e| CommandError::new(e.code()))?;
    index::update(store, |meetings| {
        let entry = meetings
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or_else(index::not_found)?;
        entry.title = title.clone();
        Ok(())
    })
    .map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => CommandError::new(NO_TRANSCRIPT),
        _ => CommandError::logged(CommandError::STORAGE_FAILED, e),
    })?;
    Ok(title)
}

#[tauri::command]
pub async fn rename_meeting<R: Runtime>(
    app: AppHandle<R>,
    meeting: String,
    title: String,
) -> Result<Option<String>, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || rename(&store, &meeting, &title))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::meeting_dir::{MeetingDir, TRANSCRIPT};
    use super::super::meeting_title::MAX_TITLE_CHARS;
    use super::*;

    const ID: &str = "2026-11-07T10:00:00Z";

    fn title(store: &FilesStore) -> Option<String> {
        index::load(store).unwrap()[0].title.clone()
    }

    #[test]
    fn title_is_trimmed_and_stored_in_the_index_only() {
        let store = crate::vault::test_store("rename-meeting");
        let id = super::super::test_meeting(&store, &super::super::test_transcript(ID));
        let path = MeetingDir::of(&id).path(store.dir()).join(TRANSCRIPT);
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            rename(&store, ID, "  Планёрка  ").unwrap().as_deref(),
            Some("Планёрка")
        );
        assert_eq!(title(&store).as_deref(), Some("Планёрка"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn empty_or_blank_title_brings_back_the_default() {
        let store = crate::vault::test_store("rename-meeting-empty");
        super::super::test_meeting(&store, &super::super::test_transcript(ID));
        for blank in ["Планёрка", "", "Планёрка", "   "] {
            let got = rename(&store, ID, blank).unwrap();
            assert_eq!(got, title(&store));
        }
        assert_eq!(title(&store), None);
    }

    #[test]
    fn too_long_title_or_control_chars_keep_the_old_title() {
        let store = crate::vault::test_store("rename-meeting-bad");
        super::super::test_meeting(&store, &super::super::test_transcript(ID));
        rename(&store, ID, "Планёрка").unwrap();
        let long = "Ж".repeat(MAX_TITLE_CHARS + 1);
        assert_eq!(
            rename(&store, ID, &long).unwrap_err().code,
            "title_too_long"
        );
        assert_eq!(
            rename(&store, ID, "a\u{0}b").unwrap_err().code,
            "title_control_chars"
        );
        assert_eq!(title(&store).as_deref(), Some("Планёрка"));
        let max = "Ж".repeat(MAX_TITLE_CHARS);
        assert_eq!(rename(&store, ID, &max).unwrap(), Some(max));
    }

    #[test]
    fn unknown_or_malformed_meeting_is_no_transcript_cwe_22() {
        let store = crate::vault::test_store("rename-meeting-unknown");
        super::super::test_meeting(&store, &super::super::test_transcript(ID));
        for bad in ["2026-11-07T10:00:01Z", "../meetings.json", ""] {
            assert_eq!(rename(&store, bad, "x").unwrap_err().code, NO_TRANSCRIPT);
        }
        assert_eq!(title(&store), None);
    }
}
