//! Удаление встречи из архива: звук, расшифровка, протокол, центры встречи в её папке и в
//! `voices.json`, строка индекса. Порядок выбран так, чтобы сбой посередине не оставил ни
//! встречу в индексе без данных, ни данные без встречи:
//! сначала центры встречи в `voices.json`, затем строка индекса, затем папка встречи.
//! Сбой на центрах или на индексе: встреча остаётся в архиве со всеми своими файлами,
//! удаление можно повторить. Папка встречи сначала переименовывается в `<папка>.deleted`
//! и только потом стирается: если стереть не вышло, по id встречу уже не найти, и ничего
//! не допишется ей до перезапуска. Строки уже нет, и папку без строки удалит подготовка
//! архива при следующем запуске.
//! Всё идёт под блокировкой правок: ни правка, ни сохранение протокола, ни «запомнить
//! голос» не запишут ничего этой встрече, пока удаление идёт.

use std::path::Path;

use tauri::{AppHandle, Runtime};

use super::meeting_dir::MeetingDir;
use super::{NO_TRANSCRIPT, edit_lock, index, meeting_id};
use crate::transcription::CommandError;
use crate::vault::FilesStore;

const DELETED_SUFFIX: &str = ".deleted";

/// `remove_dir`: как стереть папку встречи; в работе это `remove_dir_all`.
pub fn delete(
    store: &FilesStore,
    meeting: &str,
    remove_dir: &dyn Fn(&Path) -> std::io::Result<()>,
) -> Result<(), CommandError> {
    let id = meeting_id(meeting)?;
    edit_lock::locked(|| {
        // Центры встречи, которой нет в архиве, не трогаются: неизвестный id ничего не
        // стирает.
        index::find(store, &id)
            .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
            .ok_or_else(|| CommandError::new(NO_TRANSCRIPT))?;
        crate::voices::forget_meeting_centers(store, id.as_str())
            .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
        index::update(store, |meetings| {
            let before = meetings.len();
            meetings.retain(|m| m.id != id);
            if meetings.len() == before {
                return Err(index::not_found());
            }
            Ok(())
        })
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => CommandError::new(NO_TRANSCRIPT),
            _ => CommandError::logged(CommandError::STORAGE_FAILED, e),
        })?;
        remove_folder(&MeetingDir::of(&id).path(store.dir()), remove_dir);
        Ok(())
    })
}

/// Переименование в той же папке атомарно. Если переименовать не вышло, папка стирается
/// на месте: встреча видна по id, только если не прошло ни то, ни другое.
fn remove_folder(dir: &Path, remove_dir: &dyn Fn(&Path) -> std::io::Result<()>) {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(DELETED_SUFFIX);
    let doomed = dir.with_file_name(name);
    let target = match std::fs::rename(dir, &doomed) {
        Ok(()) => doomed,
        Err(e) => {
            log::warn!("папка удалённой встречи не переименована, стираю на месте: {e}");
            dir.to_path_buf()
        }
    };
    if let Err(e) = remove_dir(&target) {
        log::warn!("папка удалённой встречи не стёрта, её уберёт запуск: {e}");
    }
}

#[tauri::command]
pub async fn delete_meeting<R: Runtime>(
    app: AppHandle<R>,
    meeting: String,
) -> Result<(), CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || {
        delete(&store, &meeting, &|dir| std::fs::remove_dir_all(dir))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::meeting_dir::{AUDIO, MEETINGS_DIR};
    use super::super::meeting_id::MeetingId;
    use super::super::{
        audio_file, centers_file, startup, summary_file, test_meeting, test_transcript,
        transcript_file,
    };
    use super::*;

    const GONE: &str = "2026-11-07T10:00:00Z";
    const KEPT: &str = "2026-11-07T11:00:00Z";
    const VOICES: &str = "voices.json";

    fn remove_all(dir: &Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(dir)
    }

    /// Встреча со всеми файлами: расшифровка, центры, протокол, звук.
    fn full_meeting(store: &FilesStore, created_at: &str) -> MeetingId {
        let id = test_meeting(store, &test_transcript(created_at));
        centers_file::save(store, &id, &[(1, vec![1.0, 0.0])]).unwrap();
        summary_file::save(store, &id, "Итоги").unwrap();
        std::fs::write(MeetingDir::of(&id).path(store.dir()).join(AUDIO), b"RIFF").unwrap();
        id
    }

    fn center(meeting: &str) -> Value {
        json!({"meeting": meeting, "embedding": [0.5], "remembered_at": "2026-11-07T12:00:00Z"})
    }

    /// Анна с образцом и центрами обеих встреч, Борис только по удаляемой встрече.
    fn voices(store: &FilesStore) {
        let file = json!({"voices": [
            {"name": "Анна", "embedding": [1.0], "created_at": "2026-11-07T09:00:00Z",
             "centers": [center(GONE), center(KEPT)]},
            {"name": "Борис", "created_at": "2026-11-07T09:00:00Z", "centers": [center(GONE)]},
        ]});
        store
            .write(VOICES, &serde_json::to_vec(&file).unwrap())
            .unwrap();
    }

    fn voices_on_disk(store: &FilesStore) -> Value {
        serde_json::from_slice(&store.read(VOICES).unwrap().unwrap()).unwrap()
    }

    fn meetings_in_index(store: &FilesStore) -> Vec<String> {
        index::load(store)
            .unwrap()
            .into_iter()
            .map(|m| m.id.as_str().to_owned())
            .collect()
    }

    fn folder(store: &FilesStore, id: &MeetingId) -> std::path::PathBuf {
        MeetingDir::of(id).path(store.dir())
    }

    #[test]
    fn deletion_erases_sound_transcript_summary_centers_and_the_meeting_folder() {
        let store = crate::vault::test_store("delete-meeting-files");
        let id = full_meeting(&store, GONE);
        voices(&store);
        delete(&store, GONE, &remove_all).unwrap();
        assert_eq!(transcript_file::load(&store, &id), None);
        assert_eq!(summary_file::load(&store, &id), None);
        assert_eq!(centers_file::load(&store, &id), None);
        assert_eq!(audio_file::path(store.dir(), &id), None);
        assert!(!folder(&store, &id).exists());
        assert!(!crate::meetings::exists(&store, &id));
    }

    #[test]
    fn deletion_removes_only_this_meeting_from_the_index() {
        let store = crate::vault::test_store("delete-meeting-index");
        full_meeting(&store, GONE);
        let kept = full_meeting(&store, KEPT);
        delete(&store, GONE, &remove_all).unwrap();
        assert_eq!(meetings_in_index(&store), vec![KEPT.to_owned()]);
        assert!(transcript_file::load(&store, &kept).is_some());
        assert!(audio_file::path(store.dir(), &kept).is_some());
        assert_eq!(summary_file::load(&store, &kept).as_deref(), Some("Итоги"));
    }

    #[test]
    fn centers_of_the_meeting_leave_every_voice_and_a_voice_without_centers_stays() {
        let store = crate::vault::test_store("delete-meeting-voices");
        full_meeting(&store, GONE);
        voices(&store);
        delete(&store, GONE, &remove_all).unwrap();
        let file = voices_on_disk(&store);
        let voices = file["voices"].as_array().unwrap();
        assert_eq!(voices.len(), 2);
        assert_eq!(voices[0]["name"], "Анна");
        let meetings: Vec<&Value> = voices[0]["centers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| &c["meeting"])
            .collect();
        assert_eq!(meetings, vec![&json!(KEPT)]);
        assert_eq!(voices[0]["embedding"], json!([1.0]));
        assert_eq!(voices[1]["name"], "Борис");
        assert_eq!(voices[1]["centers"], json!([]));
    }

    #[test]
    fn unknown_or_malformed_meeting_is_no_transcript_and_touches_nothing_cwe_22() {
        let store = crate::vault::test_store("delete-meeting-unknown");
        let id = full_meeting(&store, KEPT);
        voices(&store);
        let before = voices_on_disk(&store);
        for bad in [GONE, "../meetings.json", "", "20261107T110000Z"] {
            assert_eq!(
                delete(&store, bad, &remove_all).unwrap_err().code,
                NO_TRANSCRIPT
            );
        }
        // Центры встречи, которой нет в архиве, не стираются.
        assert_eq!(voices_on_disk(&store), before);
        assert_eq!(meetings_in_index(&store), vec![KEPT.to_owned()]);
        assert!(transcript_file::load(&store, &id).is_some());
    }

    #[test]
    fn broken_voices_file_stops_deletion_before_anything_is_erased() {
        let store = crate::vault::test_store("delete-meeting-voices-broken");
        let id = full_meeting(&store, GONE);
        std::fs::write(store.dir().join(VOICES), "{\"voices\": [").unwrap();
        assert_eq!(
            delete(&store, GONE, &remove_all).unwrap_err().code,
            CommandError::STORAGE_FAILED
        );
        assert_eq!(meetings_in_index(&store), vec![GONE.to_owned()]);
        assert!(transcript_file::load(&store, &id).is_some());
        assert!(audio_file::path(store.dir(), &id).is_some());
    }

    #[test]
    fn failed_index_write_keeps_the_meeting_with_all_its_files() {
        let store = crate::vault::test_store("delete-meeting-index-fails");
        let id = full_meeting(&store, GONE);
        voices(&store);
        // Временный файл индекса занят папкой: запись индекса падает.
        std::fs::create_dir_all(store.dir().join(format!("{}.part", index::FILE))).unwrap();
        assert_eq!(
            delete(&store, GONE, &remove_all).unwrap_err().code,
            CommandError::STORAGE_FAILED
        );
        assert_eq!(meetings_in_index(&store), vec![GONE.to_owned()]);
        assert!(transcript_file::load(&store, &id).is_some());
        assert!(centers_file::load(&store, &id).is_some());
        assert_eq!(summary_file::load(&store, &id).as_deref(), Some("Итоги"));
        assert!(audio_file::path(store.dir(), &id).is_some());
        // Центры в библиотеке уже убраны: повторное удаление доводит дело до конца.
        std::fs::remove_dir(store.dir().join(format!("{}.part", index::FILE))).unwrap();
        delete(&store, GONE, &remove_all).unwrap();
        assert!(meetings_in_index(&store).is_empty());
        assert!(!folder(&store, &id).exists());
    }

    #[test]
    fn failed_folder_removal_leaves_no_index_line_and_startup_removes_the_folder() {
        let store = crate::vault::test_store("delete-meeting-folder-fails");
        let id = full_meeting(&store, GONE);
        let kept = full_meeting(&store, KEPT);
        let refuse = |_: &Path| Err(std::io::Error::other("сбой стирания"));
        delete(&store, GONE, &refuse).unwrap();
        assert_eq!(meetings_in_index(&store), vec![KEPT.to_owned()]);
        // Папка не стёрлась, но по id встречу уже не найти: ни открыть, ни дописать.
        assert_eq!(
            crate::meetings::load(&store, GONE).unwrap_err().code,
            NO_TRANSCRIPT
        );
        assert!(!crate::meetings::exists(&store, &id));
        assert_eq!(summary_file::load(&store, &id), None);
        startup::tidy(&store, &|_| {});
        assert!(!folder(&store, &id).exists());
        assert!(folder(&store, &kept).exists());
        assert_eq!(
            std::fs::read_dir(store.dir().join(MEETINGS_DIR))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn folder_that_cannot_be_renamed_is_removed_in_place() {
        let store = crate::vault::test_store("delete-meeting-rename-fails");
        let id = full_meeting(&store, GONE);
        // Непустая папка с именем цели: переименование падает.
        let doomed = store
            .dir()
            .join(MEETINGS_DIR)
            .join(format!("{}{DELETED_SUFFIX}", id.dir_name()));
        std::fs::create_dir_all(doomed.join("x")).unwrap();
        delete(&store, GONE, &remove_all).unwrap();
        assert!(!folder(&store, &id).exists());
        assert!(!crate::meetings::exists(&store, &id));
        assert!(meetings_in_index(&store).is_empty());
        startup::tidy(&store, &|_| {});
        assert!(!doomed.exists());
    }

    #[test]
    fn edit_after_deletion_writes_nothing_and_answers_no_transcript() {
        let store = crate::vault::test_store("delete-meeting-late-edit");
        let id = full_meeting(&store, GONE);
        delete(&store, GONE, &remove_all).unwrap();
        let late = crate::editing::apply(&store, GONE, |_| Ok(())).unwrap_err();
        assert_eq!(late.code, NO_TRANSCRIPT);
        assert!(!folder(&store, &id).exists());
        assert!(meetings_in_index(&store).is_empty());
    }
}
