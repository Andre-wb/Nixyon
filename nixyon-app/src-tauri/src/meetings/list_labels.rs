//! Все метки архива для листа меток и фильтра списка встреч.

use tauri::{AppHandle, Runtime};

use super::index;
use super::label::Label;
use super::label_errors::storage_failed;
use crate::transcription::CommandError;
use crate::vault::FilesStore;

/// По названию без учёта регистра; при равных названиях порядок по id, чтобы он не прыгал.
pub fn list(store: &FilesStore) -> Result<Vec<Label>, CommandError> {
    let mut labels = index::load_file(store).map_err(storage_failed)?.labels;
    labels.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    Ok(labels)
}

#[tauri::command]
pub async fn list_labels<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Label>, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || list(&store))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::create_label::create;
    use super::*;

    #[test]
    fn labels_come_sorted_by_name_ignoring_case() {
        let store = crate::vault::test_store("list-labels");
        assert_eq!(list(&store).unwrap(), Vec::new());
        for name in ["склад", "Отдел", "архив"] {
            create(&store, name).unwrap();
        }
        let names: Vec<_> = list(&store).unwrap().into_iter().map(|l| l.name).collect();
        assert_eq!(names, ["архив", "Отдел", "склад"]);
    }

    #[test]
    fn broken_index_is_an_error_not_an_empty_list() {
        let store = crate::vault::test_store("list-labels-broken");
        std::fs::write(store.dir().join(index::FILE), "{").unwrap();
        assert_eq!(list(&store).unwrap_err().code, CommandError::STORAGE_FAILED);
    }
}
