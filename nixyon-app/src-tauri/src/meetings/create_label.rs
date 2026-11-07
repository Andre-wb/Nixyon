//! Новая метка в общем списке индекса.

use tauri::{AppHandle, Runtime};

use super::index;
use super::label::Label;
use super::label_errors::{LABEL_EXISTS, storage_failed};
use super::label_name::{is_taken, parse_label_name};
use crate::transcription::CommandError;
use crate::vault::FilesStore;

/// id новой метки на единицу больше наибольшего. Удалённая метка снята со всех встреч,
/// поэтому её id можно занять снова.
pub fn create(store: &FilesStore, name: &str) -> Result<Label, CommandError> {
    let name = parse_label_name(name).map_err(|e| CommandError::new(e.code()))?;
    index::update_file(
        store,
        |file| {
            if is_taken(&file.labels, &name, None) {
                return Err(CommandError::new(LABEL_EXISTS));
            }
            let id = match file.labels.iter().map(|l| l.id).max() {
                None => 1,
                Some(max) => max.checked_add(1).ok_or_else(|| {
                    CommandError::logged(CommandError::STORAGE_FAILED, "id меток кончились")
                })?,
            };
            let label = Label { id, name };
            file.labels.push(label.clone());
            Ok(label)
        },
        storage_failed,
    )
}

#[tauri::command]
pub async fn create_label<R: Runtime>(
    app: AppHandle<R>,
    name: String,
) -> Result<Label, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || create(&store, &name))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::rename_meeting::rename;
    use super::*;

    #[test]
    fn new_label_gets_the_next_id_and_trimmed_name() {
        let store = crate::vault::test_store("create-label");
        assert_eq!(
            create(&store, "  Отдел ").unwrap(),
            Label {
                id: 1,
                name: "Отдел".into()
            }
        );
        assert_eq!(create(&store, "Склад").unwrap().id, 2);
        assert_eq!(index::load_file(&store).unwrap().labels.len(), 2);
    }

    #[test]
    fn same_name_in_another_case_or_a_bad_name_is_rejected() {
        let store = crate::vault::test_store("create-label-bad");
        create(&store, "Ёлка").unwrap();
        assert_eq!(create(&store, "ёлКА").unwrap_err().code, LABEL_EXISTS);
        assert_eq!(create(&store, "  ").unwrap_err().code, "label_empty");
        assert_eq!(
            create(&store, &"Ж".repeat(41)).unwrap_err().code,
            "label_too_long"
        );
        assert_eq!(
            create(&store, "a\nb").unwrap_err().code,
            "label_control_chars"
        );
        assert_eq!(index::load_file(&store).unwrap().labels.len(), 1);
    }

    #[test]
    fn simultaneous_label_and_meeting_changes_lose_nothing() {
        let store = crate::vault::test_store("create-label-race");
        let ids: Vec<_> = (0..8)
            .map(|i| {
                let at = format!("2026-11-07T10:00:0{i}Z");
                super::super::test_meeting(&store, &super::super::test_transcript(&at));
                at
            })
            .collect();
        std::thread::scope(|s| {
            for (i, id) in ids.iter().enumerate() {
                let store = &store;
                s.spawn(move || create(store, &format!("Метка {i}")).unwrap());
                s.spawn(move || rename(store, id, &format!("Встреча {i}")).unwrap());
            }
        });
        let file = index::load_file(&store).unwrap();
        assert_eq!(file.labels.len(), 8);
        let mut label_ids: Vec<_> = file.labels.iter().map(|l| l.id).collect();
        label_ids.sort_unstable();
        assert_eq!(label_ids, (1..=8).collect::<Vec<_>>());
        assert!(file.meetings.iter().all(|m| m.title.is_some()));
    }
}
