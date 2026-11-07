//! Архив встреч: папка на встречу в `files/meetings` и зашифрованный индекс рядом. id
//! встречи равен времени создания её расшифровки. Каждая команда, которая работает со
//! встречей, получает её id параметром: скрытой «открытой встречи» в ядре нет.

pub mod audio_file;
pub mod backfill;
pub mod centers_file;
pub mod edit_lock;
mod index;
mod index_entry;
mod legacy;
pub mod list_meetings;
mod meeting_dir;
mod meeting_id;
mod meeting_title;
pub mod open_meeting;
mod opened_meeting;
mod participant;
pub mod rename_meeting;
mod save_meeting;
mod save_transcript;
pub mod startup;
pub mod summary_file;
pub mod transcript_file;

use nixyon_core::Transcript;

use crate::transcription::CommandError;
use crate::vault::FilesStore;

pub use index::FILE as INDEX_FILE;
pub use meeting_id::MeetingId;
pub use save_meeting::{NewMeeting, save_meeting};
pub use save_transcript::save_transcript;

pub const NO_TRANSCRIPT: &str = "no_transcript";

/// id встречи из интерфейса. Неверная форма отвечает так же, как неизвестная встреча.
pub fn meeting_id(meeting: &str) -> Result<MeetingId, CommandError> {
    MeetingId::parse(meeting).ok_or_else(|| {
        log::warn!("id встречи не той формы");
        CommandError::new(NO_TRANSCRIPT)
    })
}

/// Расшифровка встречи по id из интерфейса; нет такой встречи: `no_transcript`.
pub fn load(store: &FilesStore, meeting: &str) -> Result<(MeetingId, Transcript), CommandError> {
    let id = meeting_id(meeting)?;
    let transcript =
        transcript_file::load(store, &id).ok_or_else(|| CommandError::new(NO_TRANSCRIPT))?;
    Ok((id, transcript))
}

/// Своё название встречи из индекса. Индекс не читается: встреча без своего названия,
/// причина в логе.
pub fn title(store: &FilesStore, id: &MeetingId) -> Option<String> {
    match index::find(store, id) {
        Ok(entry) => entry.and_then(|e| e.title),
        Err(e) => {
            log::warn!("название встречи не прочитано: {e}");
            None
        }
    }
}

/// Есть ли ещё встреча: фоновая работа не сохраняет результат встрече, которой уже нет.
pub fn exists(store: &FilesStore, id: &MeetingId) -> bool {
    meeting_dir::MeetingDir::of(id)
        .path(store.dir())
        .join(meeting_dir::TRANSCRIPT)
        .is_file()
}

#[cfg(test)]
pub fn test_transcript(created_at: &str) -> Transcript {
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION};
    Transcript {
        schema_version: SCHEMA_VERSION,
        created_at: created_at.into(),
        audio: AudioInfo {
            file: "a.wav".into(),
            duration_sec: 1.5,
            sample_rate: 16_000,
        },
        engine: EngineInfo {
            vad: "vad.onnx".into(),
            asr: "v1".into(),
            embedding: "embedding.onnx".into(),
        },
        speakers: Vec::new(),
        turns: Vec::new(),
    }
}

/// Встреча в архиве для тестов команд: папка, расшифровка, строка индекса.
#[cfg(test)]
pub fn test_meeting(store: &FilesStore, transcript: &Transcript) -> MeetingId {
    let id = MeetingId::parse(&transcript.created_at).expect("created_at теста формы id");
    std::fs::create_dir_all(meeting_dir::MeetingDir::of(&id).path(store.dir())).unwrap();
    transcript_file::save(store, &id, transcript).unwrap();
    index::update(store, |meetings| {
        meetings.push(index_entry::IndexEntry {
            id: id.clone(),
            duration_sec: transcript.audio.duration_sec,
            file: transcript.audio.file.clone(),
            title: None,
            participants: Some(participant::participants(transcript)),
        });
        Ok(())
    })
    .unwrap();
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meeting_is_loaded_by_id_and_unknown_or_malformed_id_is_no_transcript() {
        let store = crate::vault::test_store("meetings-load");
        let first = test_transcript("2026-11-07T10:00:00Z");
        let second = test_transcript("2026-11-07T11:00:00Z");
        let other = test_meeting(&store, &second);
        assert!(exists(&store, &other));
        let id = test_meeting(&store, &first);
        assert_eq!(
            load(&store, "2026-11-07T10:00:00Z").unwrap(),
            (id.clone(), first)
        );
        assert!(exists(&store, &id));
        for bad in ["2026-11-07T12:00:00Z", "../meetings.json", ""] {
            assert_eq!(load(&store, bad).unwrap_err().code, NO_TRANSCRIPT);
        }
    }
}
