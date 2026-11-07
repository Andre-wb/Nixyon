//! Архив при запуске, до любой работы с файлами: старые файлы последней встречи
//! удаляются, папка встречи без строки в индексе (сбой посередине сохранения) удаляется,
//! в остальных папках убираются оборванные `.part`, а каждый файл проходит `secure`:
//! открытый шифруется, под потерянным ключом удаляется. Если индекс не читается, ни одна
//! папка не удаляется: без индекса нельзя отличить остаток сбоя от встречи.

use std::collections::HashSet;
use std::path::Path;

use super::index;
use super::legacy;
use super::meeting_dir::MEETINGS_DIR;
use crate::vault::FilesStore;

pub fn tidy(store: &FilesStore, secure: &dyn Fn(&Path)) {
    legacy::remove(store.dir());
    let Ok(entries) = std::fs::read_dir(store.dir().join(MEETINGS_DIR)) else {
        return;
    };
    let known: Option<HashSet<String>> = match index::load(store) {
        Ok(meetings) => Some(meetings.iter().map(|m| m.id.dir_name()).collect()),
        Err(e) => {
            log::error!("индекс архива не читается, папки встреч не трогаю: {e}");
            None
        }
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        match &known {
            Some(known) if !known.contains(&name) => match std::fs::remove_dir_all(entry.path()) {
                Ok(()) => log::warn!("папка встречи {name} без строки в индексе удалена"),
                Err(e) => log::warn!("папка встречи {name} без строки в индексе не удалена: {e}"),
            },
            _ => tidy_files(&entry.path(), secure),
        }
    }
}

fn tidy_files(dir: &Path, secure: &dyn Fn(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if entry.file_name().to_string_lossy().ends_with(".part") {
            if let Err(e) = std::fs::remove_file(&path) {
                log::warn!("оборванный файл встречи не удалён: {e}");
            }
        } else {
            secure(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use super::super::index_entry::IndexEntry;
    use super::super::meeting_dir::{MeetingDir, TRANSCRIPT};
    use super::super::meeting_id::MeetingId;
    use super::*;

    fn folder(store: &FilesStore, secs: u64) -> PathBuf {
        let dir = MeetingDir::of(&MeetingId::from_unix_secs(secs)).path(store.dir());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(TRANSCRIPT), b"{}").unwrap();
        std::fs::write(dir.join("transcript.json.part"), b"{").unwrap();
        dir
    }

    #[test]
    fn folder_without_an_index_line_is_removed_and_known_ones_are_secured() {
        let store = crate::vault::test_store("meetings-startup");
        let known = folder(&store, 100);
        let orphan = folder(&store, 200);
        std::fs::write(store.dir().join("last_audio.txt"), b"/data/rec.wav").unwrap();
        index::update(&store, |m| {
            m.push(IndexEntry {
                id: MeetingId::from_unix_secs(100),
                duration_sec: 1.0,
                file: "a.wav".into(),
                title: None,
                participants: None,
                labels: Vec::new(),
            });
            Ok(())
        })
        .unwrap();
        let secured = RefCell::new(Vec::new());
        tidy(&store, &|p| secured.borrow_mut().push(p.to_path_buf()));
        assert!(known.join(TRANSCRIPT).exists());
        assert!(!known.join("transcript.json.part").exists());
        assert!(!orphan.exists());
        assert_eq!(secured.into_inner(), vec![known.join(TRANSCRIPT)]);
        assert!(!store.dir().join("last_audio.txt").exists());
    }

    #[test]
    fn broken_index_removes_no_meeting_folder() {
        let store = crate::vault::test_store("meetings-startup-broken");
        let first = folder(&store, 100);
        let second = folder(&store, 200);
        std::fs::write(store.dir().join(index::FILE), "{").unwrap();
        let secured = RefCell::new(0);
        tidy(&store, &|_| *secured.borrow_mut() += 1);
        assert!(first.join(TRANSCRIPT).exists());
        assert!(second.join(TRANSCRIPT).exists());
        assert_eq!(secured.into_inner(), 2);
    }
}
