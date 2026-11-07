//! Новое название метки. Встречи ссылаются на метку по id, поэтому меняется одно место.

use tauri::{AppHandle, Runtime};

use super::index;
use super::label::Label;
use super::label_errors::{LABEL_EXISTS, NO_LABEL, storage_failed};
use super::label_name::{is_taken, parse_label_name};
use crate::transcription::CommandError;
use crate::vault::FilesStore;

pub fn rename(store: &FilesStore, label: u32, name: &str) -> Result<Label, CommandError> {
    let name = parse_label_name(name).map_err(|e| CommandError::new(e.code()))?;
    index::update_file(
        store,
        |file| {
            if is_taken(&file.labels, &name, Some(label)) {
                return Err(CommandError::new(LABEL_EXISTS));
            }
            let found = file
                .labels
                .iter_mut()
                .find(|l| l.id == label)
                .ok_or_else(|| CommandError::new(NO_LABEL))?;
            found.name = name;
            Ok(found.clone())
        },
        storage_failed,
    )
}

#[tauri::command]
pub async fn rename_label<R: Runtime>(
    app: AppHandle<R>,
    label: u32,
    name: String,
) -> Result<Label, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || rename(&store, label, &name))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::create_label::create;
    use super::super::set_meeting_labels::set;
    use super::*;

    const ID: &str = "2026-11-07T10:00:00Z";

    #[test]
    fn renamed_label_stays_on_its_meetings() {
        let store = crate::vault::test_store("rename-label");
        super::super::test_meeting(&store, &super::super::test_transcript(ID));
        let label = create(&store, "Отдел").unwrap();
        set(&store, ID, &[label.id]).unwrap();
        assert_eq!(
            rename(&store, label.id, " Отдел продаж ").unwrap(),
            Label {
                id: label.id,
                name: "Отдел продаж".into()
            }
        );
        let file = index::load_file(&store).unwrap();
        assert_eq!(file.labels[0].name, "Отдел продаж");
        assert_eq!(file.meetings[0].labels, [label.id]);
    }

    #[test]
    fn case_change_of_the_same_label_is_allowed_and_a_taken_name_is_not() {
        let store = crate::vault::test_store("rename-label-taken");
        let a = create(&store, "отдел").unwrap();
        create(&store, "Склад").unwrap();
        assert_eq!(rename(&store, a.id, "Отдел").unwrap().name, "Отдел");
        assert_eq!(
            rename(&store, a.id, "склад").unwrap_err().code,
            LABEL_EXISTS
        );
        assert_eq!(rename(&store, a.id, "").unwrap_err().code, "label_empty");
        assert_eq!(rename(&store, 99, "Новая").unwrap_err().code, NO_LABEL);
        assert_eq!(index::load_file(&store).unwrap().labels[0].name, "Отдел");
    }
}
