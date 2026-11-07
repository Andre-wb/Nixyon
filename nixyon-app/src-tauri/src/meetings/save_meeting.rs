//! Сохранение новой встречи в архив. Порядок: папка, расшифровка и центры, строка индекса,
//! затем звук. Встреча появляется в архиве только со строкой индекса; папку без строки
//! (сбой посередине) удаляет подготовка при запуске. Звук переносится последним: если
//! индекс не записался, запись остаётся в `recordings`, а не пропадает вместе с папкой.

use std::path::Path;

use nixyon_core::Transcript;
use nixyon_pipeline::SpeakerCenter;

use super::index_entry::IndexEntry;
use super::meeting_dir::MeetingDir;
use super::meeting_id::MeetingId;
use super::participant::participants;
use super::{audio_file, centers_file, index, transcript_file};
use crate::vault::FilesStore;

/// Новая встреча из готовой расшифровки. `created_secs`: время создания; если эта секунда
/// уже занята другой встречей, берётся следующая свободная, и `created_at` расшифровки
/// совпадает с id встречи.
pub struct NewMeeting<'a> {
    pub transcript: Transcript,
    pub centers: &'a [SpeakerCenter],
    pub audio: &'a Path,
    pub created_secs: u64,
}

pub fn save_meeting(store: &FilesStore, meeting: NewMeeting<'_>) -> std::io::Result<Transcript> {
    let NewMeeting {
        mut transcript,
        centers,
        audio,
        created_secs,
    } = meeting;
    // Папка, которую успели создать: если индекс не записался, её нужно убрать.
    let mut created = None;
    let written = index::update(store, |meetings| {
        let id = free_id(store.dir(), meetings, created_secs);
        transcript.created_at = id.as_str().to_owned();
        let dir = MeetingDir::of(&id).path(store.dir());
        std::fs::create_dir_all(&dir)?;
        created = Some(dir);
        transcript_file::save(store, &id, &transcript)?;
        if let Err(e) = centers_file::save(store, &id, centers) {
            log::warn!("центры голосов не сохранены, запомнить голос будет нельзя: {e}");
        }
        meetings.push(IndexEntry {
            id: id.clone(),
            duration_sec: transcript.audio.duration_sec,
            file: transcript.audio.file.clone(),
            title: None,
            participants: Some(participants(&transcript)),
        });
        Ok(id)
    });
    let id = match written {
        Ok(id) => id,
        Err(e) => {
            if let Some(dir) = created
                && let Err(e) = std::fs::remove_dir_all(&dir)
            {
                log::warn!("папка несохранённой встречи не удалена: {e}");
            }
            return Err(e);
        }
    };
    if let Err(e) = audio_file::move_in(store.dir(), &id, audio) {
        log::warn!("запись не перенесена к встрече, проигрывать будет нечего: {e}");
    }
    Ok(transcript)
}

fn free_id(files_dir: &Path, meetings: &[IndexEntry], from_secs: u64) -> MeetingId {
    let mut secs = from_secs;
    loop {
        let id = MeetingId::from_unix_secs(secs);
        let taken =
            meetings.iter().any(|m| m.id == id) || MeetingDir::of(&id).path(files_dir).exists();
        if !taken {
            return id;
        }
        secs += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::super::meeting_dir::{AUDIO, MEETINGS_DIR};
    use super::*;

    fn recording(store: &FilesStore, name: &str) -> std::path::PathBuf {
        let rec = store.dir().join("recordings");
        std::fs::create_dir_all(&rec).unwrap();
        let path = rec.join(name);
        std::fs::write(&path, b"RIFF").unwrap();
        path
    }

    fn save(store: &FilesStore, secs: u64, audio: &Path) -> std::io::Result<Transcript> {
        save_meeting(
            store,
            NewMeeting {
                transcript: super::super::test_transcript("1970-01-01T00:00:00Z"),
                centers: &[(1, vec![1.0, 0.0])],
                audio,
                created_secs: secs,
            },
        )
    }

    #[test]
    fn meeting_gets_its_folder_index_line_and_recording() {
        let store = crate::vault::test_store("save-meeting");
        let audio = recording(&store, "rec-1.wav");
        let saved = save(&store, 1_000, &audio).unwrap();
        let id = MeetingId::from_unix_secs(1_000);
        assert_eq!(saved.created_at, id.as_str());
        assert_eq!(transcript_file::load(&store, &id), Some(saved));
        assert_eq!(centers_file::load(&store, &id).unwrap().len(), 1);
        let entry = &index::load(&store).unwrap()[0];
        assert_eq!(entry.id, id);
        assert_eq!(entry.title, None);
        assert_eq!(entry.participants, Some(Vec::new()));
        assert!(!audio.exists());
        assert!(audio_file::path(store.dir(), &id).is_some());
    }

    #[test]
    fn second_meeting_in_the_same_second_takes_the_next_free_second() {
        let store = crate::vault::test_store("save-meeting-same-second");
        let first = save(&store, 1_000, &recording(&store, "rec-1.wav")).unwrap();
        let second = save(&store, 1_000, &recording(&store, "rec-2.wav")).unwrap();
        let third = save(&store, 1_000, &recording(&store, "rec-3.wav")).unwrap();
        assert_eq!(first.created_at, "1970-01-01T00:16:40Z");
        assert_eq!(second.created_at, "1970-01-01T00:16:41Z");
        assert_eq!(third.created_at, "1970-01-01T00:16:42Z");
        assert_eq!(index::load(&store).unwrap().len(), 3);
    }

    #[test]
    fn recording_that_did_not_move_leaves_the_meeting_without_sound() {
        let store = crate::vault::test_store("save-meeting-no-audio");
        let gone = store.dir().join("recordings/нет.wav");
        let saved = save(&store, 1_000, &gone).unwrap();
        let id = MeetingId::parse(&saved.created_at).unwrap();
        assert!(transcript_file::load(&store, &id).is_some());
        assert_eq!(audio_file::path(store.dir(), &id), None);
        assert!(!MeetingDir::of(&id).path(store.dir()).join(AUDIO).exists());
    }

    #[test]
    fn failed_index_write_leaves_no_meeting_folder_and_keeps_the_recording() {
        let store = crate::vault::test_store("save-meeting-index-fails");
        let audio = recording(&store, "rec-1.wav");
        // Временный файл индекса занят папкой: запись индекса падает.
        std::fs::create_dir_all(store.dir().join(format!("{}.part", index::FILE))).unwrap();
        assert!(save(&store, 1_000, &audio).is_err());
        assert!(audio.exists());
        let left = std::fs::read_dir(store.dir().join(MEETINGS_DIR))
            .unwrap()
            .count();
        assert_eq!(left, 0);
    }

    #[test]
    fn broken_index_is_not_overwritten_by_a_new_meeting() {
        let store = crate::vault::test_store("save-meeting-broken-index");
        std::fs::write(store.dir().join(index::FILE), "{").unwrap();
        let audio = recording(&store, "rec-1.wav");
        assert!(save(&store, 1_000, &audio).is_err());
        assert_eq!(std::fs::read(store.dir().join(index::FILE)).unwrap(), b"{");
        assert!(audio.exists());
    }
}
