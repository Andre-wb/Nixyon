//! Метки одной встречи: интерфейс присылает полный набор, он заменяет прежний.

use tauri::{AppHandle, Runtime};

use super::index;
use super::label_errors::{NO_LABEL, storage_failed};
use super::{NO_TRANSCRIPT, meeting_id};
use crate::transcription::CommandError;
use crate::vault::FilesStore;

/// Набор хранится по возрастанию id и без повторов. Неизвестная метка не назначается
/// вовсе: набор встречи остаётся прежним.
pub fn set(store: &FilesStore, meeting: &str, labels: &[u32]) -> Result<Vec<u32>, CommandError> {
    let id = meeting_id(meeting)?;
    let mut labels = labels.to_vec();
    labels.sort_unstable();
    labels.dedup();
    index::update_file(
        store,
        |file| {
            if !labels
                .iter()
                .all(|l| file.labels.iter().any(|k| k.id == *l))
            {
                return Err(CommandError::new(NO_LABEL));
            }
            let entry = file
                .meetings
                .iter_mut()
                .find(|m| m.id == id)
                .ok_or_else(|| CommandError::new(NO_TRANSCRIPT))?;
            entry.labels = labels.clone();
            Ok(labels)
        },
        storage_failed,
    )
}

#[tauri::command]
pub async fn set_meeting_labels<R: Runtime>(
    app: AppHandle<R>,
    meeting: String,
    labels: Vec<u32>,
) -> Result<Vec<u32>, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || set(&store, &meeting, &labels))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::create_label::create;
    use super::*;

    const ID: &str = "2026-11-07T10:00:00Z";

    fn labels_of(store: &FilesStore) -> Vec<u32> {
        index::load(store).unwrap()[0].labels.clone()
    }

    #[test]
    fn labels_are_assigned_and_removed_as_a_whole_set() {
        let store = crate::vault::test_store("set-meeting-labels");
        super::super::test_meeting(&store, &super::super::test_transcript(ID));
        let a = create(&store, "Отдел").unwrap().id;
        let b = create(&store, "Склад").unwrap().id;
        assert_eq!(set(&store, ID, &[b, a, b]).unwrap(), [a, b]);
        assert_eq!(labels_of(&store), [a, b]);
        assert_eq!(set(&store, ID, &[b]).unwrap(), [b]);
        assert_eq!(set(&store, ID, &[]).unwrap(), Vec::<u32>::new());
        assert_eq!(labels_of(&store), Vec::<u32>::new());
    }

    #[test]
    fn unknown_label_or_meeting_changes_nothing() {
        let store = crate::vault::test_store("set-meeting-labels-unknown");
        super::super::test_meeting(&store, &super::super::test_transcript(ID));
        let a = create(&store, "Отдел").unwrap().id;
        set(&store, ID, &[a]).unwrap();
        assert_eq!(set(&store, ID, &[a, 99]).unwrap_err().code, NO_LABEL);
        assert_eq!(labels_of(&store), [a]);
        for bad in ["2026-11-07T10:00:01Z", "../meetings.json"] {
            assert_eq!(set(&store, bad, &[a]).unwrap_err().code, NO_TRANSCRIPT);
        }
    }
}
