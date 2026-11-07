//! Список встреч архива для главного экрана, новые сверху.

use tauri::{AppHandle, Runtime};

use super::index;
use super::index_entry::IndexEntry;
use crate::transcription::CommandError;
use crate::vault::FilesStore;

/// id встреч разные и одной ширины, поэтому порядок по ним однозначный и совпадает со
/// временем.
pub fn list(store: &FilesStore) -> std::io::Result<Vec<IndexEntry>> {
    let mut meetings = index::load(store)?;
    meetings.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(meetings)
}

#[tauri::command]
pub async fn list_meetings<R: Runtime>(app: AppHandle<R>) -> Result<Vec<IndexEntry>, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || list(&store))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_meeting_comes_first_and_empty_archive_is_an_empty_list() {
        let store = crate::vault::test_store("list-meetings");
        assert_eq!(list(&store).unwrap(), Vec::new());
        for at in [
            "2026-11-07T10:00:00Z",
            "2026-11-08T09:00:00Z",
            "2025-12-31T23:59:59Z",
        ] {
            super::super::test_meeting(&store, &super::super::test_transcript(at));
        }
        let ids: Vec<_> = list(&store)
            .unwrap()
            .into_iter()
            .map(|m| m.id.as_str().to_owned())
            .collect();
        assert_eq!(
            ids,
            [
                "2026-11-08T09:00:00Z",
                "2026-11-07T10:00:00Z",
                "2025-12-31T23:59:59Z"
            ]
        );
    }

    #[test]
    fn broken_index_is_an_error_not_an_empty_list() {
        let store = crate::vault::test_store("list-meetings-broken");
        std::fs::write(store.dir().join(index::FILE), "{").unwrap();
        assert!(list(&store).is_err());
    }
}
