//! Расшифровка встречи в её папке, зашифрована. JSON тот же, что в экспорте.

use nixyon_core::{Transcript, from_json, to_json};

use super::meeting_dir::{MeetingDir, TRANSCRIPT};
use super::meeting_id::MeetingId;
use crate::vault::FilesStore;

/// Правка заменяет расшифровку целиком или не заменяет вовсе: оборванная запись не оставит
/// битый файл.
pub fn save(store: &FilesStore, id: &MeetingId, transcript: &Transcript) -> std::io::Result<()> {
    let json = to_json(transcript).map_err(std::io::Error::other)?;
    store.write(&MeetingDir::of(id).file(TRANSCRIPT), json.as_bytes())
}

/// Расшифровка встречи, если она есть и читается.
pub fn load(store: &FilesStore, id: &MeetingId) -> Option<Transcript> {
    let bytes = match store.read(&MeetingDir::of(id).file(TRANSCRIPT)) {
        Ok(bytes) => bytes?,
        Err(e) => {
            log::error!("расшифровка встречи не читается: {e}");
            return None;
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => {
            log::error!("расшифровка встречи не читается: {e}");
            return None;
        }
    };
    match from_json(&text) {
        Ok(t) => Some(t),
        Err(e) => {
            log::error!("расшифровка встречи не читается: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_is_kept_sealed_in_the_meeting_folder() {
        let store = crate::vault::test_store("meeting-transcript");
        let id = MeetingId::parse("2026-10-28T12:00:00Z").unwrap();
        let other = MeetingId::parse("2026-10-28T12:00:01Z").unwrap();
        assert!(load(&store, &id).is_none());
        std::fs::create_dir_all(MeetingDir::of(&id).path(store.dir())).unwrap();
        let mut t = super::super::test_transcript(id.as_str());
        t.audio.file = "b.wav".into();
        save(&store, &id, &t).unwrap();
        assert_eq!(load(&store, &id).unwrap().audio.file, "b.wav");
        assert!(load(&store, &other).is_none());
        let path = MeetingDir::of(&id).path(store.dir()).join(TRANSCRIPT);
        assert!(
            !std::fs::read(&path)
                .unwrap()
                .windows(5)
                .any(|w| w == b"b.wav")
        );
        std::fs::write(&path, "{").unwrap();
        assert!(load(&store, &id).is_none());
    }
}
