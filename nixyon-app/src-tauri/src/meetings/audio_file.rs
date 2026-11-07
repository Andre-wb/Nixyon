//! Исходная запись встречи в её папке. Kotlin пишет запись уже зашифрованной в
//! `recordings`, при сохранении встречи она переносится сюда переименованием: обе папки
//! внутри `files`, перенос атомарный.

use std::path::{Path, PathBuf};

use super::meeting_dir::{AUDIO, MeetingDir};
use super::meeting_id::MeetingId;

pub fn move_in(files_dir: &Path, id: &MeetingId, source: &Path) -> std::io::Result<()> {
    std::fs::rename(source, MeetingDir::of(id).path(files_dir).join(AUDIO))
}

/// Запись встречи, если она есть: у встречи без звука проигрывать нечего.
pub fn path(files_dir: &Path, id: &MeetingId) -> Option<PathBuf> {
    let path = MeetingDir::of(id).path(files_dir).join(AUDIO);
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_moves_into_the_meeting_folder() {
        let store = crate::vault::test_store("meeting-audio");
        let id = MeetingId::parse("2026-10-30T10:00:00Z").unwrap();
        std::fs::create_dir_all(MeetingDir::of(&id).path(store.dir())).unwrap();
        let source = store.dir().join("rec-1.wav");
        std::fs::write(&source, b"RIFF").unwrap();
        assert_eq!(path(store.dir(), &id), None);
        move_in(store.dir(), &id, &source).unwrap();
        assert!(!source.exists());
        let moved = path(store.dir(), &id).unwrap();
        assert_eq!(std::fs::read(moved).unwrap(), b"RIFF");
    }
}
