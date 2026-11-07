//! Удаление метки: она уходит из общего списка и снимается со всех встреч. Сами встречи
//! остаются.

use tauri::{AppHandle, Runtime};

use super::index;
use super::label_errors::{NO_LABEL, storage_failed};
use crate::transcription::CommandError;
use crate::vault::FilesStore;

pub fn delete(store: &FilesStore, label: u32) -> Result<(), CommandError> {
    index::update_file(
        store,
        |file| {
            let before = file.labels.len();
            file.labels.retain(|l| l.id != label);
            if file.labels.len() == before {
                return Err(CommandError::new(NO_LABEL));
            }
            for meeting in &mut file.meetings {
                meeting.labels.retain(|&l| l != label);
            }
            Ok(())
        },
        storage_failed,
    )
}

#[tauri::command]
pub async fn delete_label<R: Runtime>(app: AppHandle<R>, label: u32) -> Result<(), CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || delete(&store, label))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::create_label::create;
    use super::super::set_meeting_labels::set;
    use super::*;

    #[test]
    fn assigned_label_is_removed_from_every_meeting_and_meetings_stay() {
        let store = crate::vault::test_store("delete-label");
        let ids = ["2026-11-07T10:00:00Z", "2026-11-07T11:00:00Z"];
        for id in ids {
            super::super::test_meeting(&store, &super::super::test_transcript(id));
        }
        let gone = create(&store, "Отдел").unwrap();
        let kept = create(&store, "Склад").unwrap();
        for id in ids {
            set(&store, id, &[gone.id, kept.id]).unwrap();
        }
        delete(&store, gone.id).unwrap();
        let file = index::load_file(&store).unwrap();
        assert_eq!(file.labels, std::slice::from_ref(&kept));
        assert_eq!(file.meetings.len(), 2);
        assert!(file.meetings.iter().all(|m| m.labels == [kept.id]));
        assert_eq!(delete(&store, gone.id).unwrap_err().code, NO_LABEL);
    }
}
