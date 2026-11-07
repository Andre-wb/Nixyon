//! Индекс архива встреч в `files/meetings.json`, зашифрован. Встреча есть в архиве, только
//! когда у неё есть строка здесь.

use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use super::index_entry::IndexEntry;
use super::meeting_id::MeetingId;
use crate::vault::FilesStore;

pub const FILE: &str = "meetings.json";

/// Индекс меняют сохранение встречи, переименование и правки участников, в том числе
/// одновременно из разных команд. Без общей блокировки на весь цикл «прочитать, изменить,
/// записать» одна из двух записей потерялась бы.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct IndexFile {
    meetings: Vec<IndexEntry>,
}

/// Нет файла: архив пуст. Битый файл: ошибка с видом `InvalidData`, чтобы следующее
/// сохранение не затёрло встречи.
pub fn load(store: &FilesStore) -> std::io::Result<Vec<IndexEntry>> {
    match store.read(FILE)? {
        Some(bytes) => serde_json::from_slice::<IndexFile>(&bytes)
            .map(|f| f.meetings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        None => Ok(Vec::new()),
    }
}

/// Оборванная запись не портит прежний индекс.
fn save(store: &FilesStore, meetings: &[IndexEntry]) -> std::io::Result<()> {
    let json = serde_json::to_vec(&IndexFile {
        meetings: meetings.to_vec(),
    })
    .map_err(std::io::Error::other)?;
    store.write(FILE, &json)
}

/// Читает индекс, даёт его изменить и записывает. Ошибка изменения оставляет индекс как
/// был.
pub fn update<T>(
    store: &FilesStore,
    change: impl FnOnce(&mut Vec<IndexEntry>) -> std::io::Result<T>,
) -> std::io::Result<T> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let mut meetings = load(store)?;
    let result = change(&mut meetings)?;
    save(store, &meetings)?;
    Ok(result)
}

/// Строка встречи, если она есть в индексе.
pub fn find(store: &FilesStore, id: &MeetingId) -> std::io::Result<Option<IndexEntry>> {
    Ok(load(store)?.into_iter().find(|m| &m.id == id))
}

/// Нет строки встречи в индексе: команда отвечает `no_transcript`.
pub fn not_found() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, "встречи нет в индексе")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(secs: u64) -> IndexEntry {
        IndexEntry {
            id: MeetingId::from_unix_secs(secs),
            duration_sec: 2.5,
            file: "встреча.m4a".into(),
            title: None,
            participants: Some(Vec::new()),
        }
    }

    #[test]
    fn index_is_saved_sealed_and_read_back() {
        let store = crate::vault::test_store("meetings-index");
        assert_eq!(load(&store).unwrap(), Vec::new());
        let meetings = vec![entry(200), entry(1_000_000), entry(5)];
        update(&store, |m| {
            m.extend(meetings.iter().cloned());
            Ok(())
        })
        .unwrap();
        assert_eq!(load(&store).unwrap(), meetings);
        assert_eq!(
            find(&store, &MeetingId::from_unix_secs(5)).unwrap(),
            Some(meetings[2].clone())
        );
        assert_eq!(find(&store, &MeetingId::from_unix_secs(6)).unwrap(), None);
        let on_disk = std::fs::read(store.dir().join(FILE)).unwrap();
        assert!(
            !on_disk
                .windows("встреча".len())
                .any(|w| w == "встреча".as_bytes())
        );
    }

    #[test]
    fn index_of_the_previous_version_without_title_and_participants_is_read() {
        let store = crate::vault::test_store("meetings-index-previous");
        let json =
            r#"{"meetings":[{"id":"2026-11-07T10:00:00Z","duration_sec":1.5,"file":"a.wav"}]}"#;
        store.write(FILE, json.as_bytes()).unwrap();
        let meetings = load(&store).unwrap();
        assert_eq!(meetings.len(), 1);
        assert_eq!(meetings[0].title, None);
        assert_eq!(meetings[0].participants, None);
    }

    #[test]
    fn failed_change_leaves_the_index_as_it_was() {
        let store = crate::vault::test_store("meetings-index-failed-change");
        update(&store, |m| {
            m.push(entry(1));
            Ok(())
        })
        .unwrap();
        let err = update(&store, |m| {
            m.clear();
            Err::<(), _>(not_found())
        })
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert_eq!(load(&store).unwrap(), vec![entry(1)]);
    }

    #[test]
    fn simultaneous_changes_lose_no_meeting() {
        let store = crate::vault::test_store("meetings-index-race");
        std::thread::scope(|s| {
            for secs in 0..16 {
                let store = &store;
                s.spawn(move || {
                    update(store, |m| {
                        // Пауза между чтением и записью: без блокировки все потоки успели
                        // бы прочитать один и тот же индекс.
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        m.push(entry(secs));
                        Ok(())
                    })
                    .unwrap()
                });
            }
        });
        let mut ids: Vec<_> = load(&store).unwrap().into_iter().map(|m| m.id).collect();
        ids.sort();
        let expected: Vec<_> = (0..16).map(MeetingId::from_unix_secs).collect();
        assert_eq!(ids, expected);
    }

    #[test]
    fn broken_index_or_a_bad_id_inside_is_an_error_not_an_empty_archive() {
        let store = crate::vault::test_store("meetings-index-broken");
        std::fs::write(store.dir().join(FILE), "{\"meetings\": [").unwrap();
        assert_eq!(
            load(&store).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        std::fs::write(
            store.dir().join(FILE),
            r#"{"meetings":[{"id":"../x","duration_sec":1.0,"file":"a"}]}"#,
        )
        .unwrap();
        assert_eq!(
            load(&store).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        assert!(
            update(&store, |m| {
                m.clear();
                Ok(())
            })
            .is_err()
        );
    }
}
