//! Протокол встречи, составленный моделью, в её папке. Личные данные: файл зашифрован, в
//! лог текст не пишется.

use serde::{Deserialize, Serialize};

use super::meeting_dir::{MeetingDir, SUMMARY};
use super::meeting_id::MeetingId;
use crate::vault::FilesStore;

#[derive(Serialize, Deserialize)]
struct SummaryFile {
    text: String,
}

pub fn save(store: &FilesStore, id: &MeetingId, text: &str) -> std::io::Result<()> {
    let json = serde_json::to_vec(&SummaryFile {
        text: text.to_owned(),
    })
    .map_err(std::io::Error::other)?;
    store.write(&MeetingDir::of(id).file(SUMMARY), &json)
}

/// Протокол встречи, если он есть и читается.
pub fn load(store: &FilesStore, id: &MeetingId) -> Option<String> {
    let bytes = match store.read(&MeetingDir::of(id).file(SUMMARY)) {
        Ok(bytes) => bytes?,
        Err(e) => {
            log::error!("протокол встречи не читается: {e}");
            return None;
        }
    };
    match serde_json::from_slice::<SummaryFile>(&bytes) {
        Ok(file) => Some(file.text),
        Err(e) => {
            log::error!("протокол встречи не читается: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_belongs_to_its_own_meeting_and_is_sealed() {
        let store = crate::vault::test_store("meeting-summary");
        let first = MeetingId::parse("2026-10-30T10:00:00Z").unwrap();
        let second = MeetingId::parse("2026-10-30T11:00:00Z").unwrap();
        for id in [&first, &second] {
            std::fs::create_dir_all(MeetingDir::of(id).path(store.dir())).unwrap();
        }
        assert!(load(&store, &first).is_none());
        save(&store, &first, "Задачи: - Анна: позвонить").unwrap();
        assert_eq!(
            load(&store, &first).as_deref(),
            Some("Задачи: - Анна: позвонить")
        );
        assert!(load(&store, &second).is_none());
        let path = MeetingDir::of(&first).path(store.dir()).join(SUMMARY);
        let on_disk = std::fs::read(&path).unwrap();
        assert!(
            !on_disk
                .windows("Анна".len())
                .any(|w| w == "Анна".as_bytes())
        );
        std::fs::write(&path, "{").unwrap();
        assert!(load(&store, &first).is_none());
    }
}
