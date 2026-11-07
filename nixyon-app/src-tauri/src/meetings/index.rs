//! Индекс архива встреч в `files/meetings.json`, зашифрован. Встреча есть в архиве, только
//! когда у неё есть строка здесь.

use serde::{Deserialize, Serialize};

use super::index_entry::IndexEntry;
use crate::vault::FilesStore;

pub const FILE: &str = "meetings.json";

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
pub fn save(store: &FilesStore, meetings: &[IndexEntry]) -> std::io::Result<()> {
    let json = serde_json::to_vec(&IndexFile {
        meetings: meetings.to_vec(),
    })
    .map_err(std::io::Error::other)?;
    store.write(FILE, &json)
}

/// Самая поздняя встреча: id одной ширины, и строки сравниваются как время.
pub fn latest(meetings: &[IndexEntry]) -> Option<&IndexEntry> {
    meetings.iter().max_by(|a, b| a.id.cmp(&b.id))
}

#[cfg(test)]
mod tests {
    use super::super::meeting_id::MeetingId;
    use super::*;

    fn entry(secs: u64) -> IndexEntry {
        IndexEntry {
            id: MeetingId::from_unix_secs(secs),
            duration_sec: 2.5,
            file: "встреча.m4a".into(),
        }
    }

    #[test]
    fn index_is_saved_sealed_and_latest_is_the_greatest_id() {
        let store = crate::vault::test_store("meetings-index");
        assert_eq!(load(&store).unwrap(), Vec::new());
        let meetings = vec![entry(200), entry(1_000_000), entry(5)];
        save(&store, &meetings).unwrap();
        assert_eq!(load(&store).unwrap(), meetings);
        assert_eq!(latest(&meetings), Some(&meetings[1]));
        assert_eq!(latest(&[]), None);
        let on_disk = std::fs::read(store.dir().join(FILE)).unwrap();
        assert!(
            !on_disk
                .windows("встреча".len())
                .any(|w| w == "встреча".as_bytes())
        );
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
    }
}
