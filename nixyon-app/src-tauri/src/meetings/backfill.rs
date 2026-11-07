//! Участники у встреч прежней версии: в их строках индекса поля ещё нет. При запуске оно
//! заполняется из расшифровок одной записью индекса. Нечитаемая расшифровка оставляет
//! строку без участников, следующий запуск попробует снова.

use super::index;
use super::participant::participants;
use super::transcript_file;
use crate::vault::FilesStore;

pub fn fill_participants(store: &FilesStore) {
    match index::load(store) {
        Ok(meetings) if meetings.iter().any(|m| m.participants.is_none()) => {}
        Ok(_) => return,
        Err(e) => {
            log::error!("индекс архива не читается, участников не заполняю: {e}");
            return;
        }
    }
    let filled = index::update(store, |meetings| {
        for entry in meetings.iter_mut().filter(|m| m.participants.is_none()) {
            match transcript_file::load(store, &entry.id) {
                Some(transcript) => entry.participants = Some(participants(&transcript)),
                None => log::warn!("участники встречи не заполнены: расшифровка не читается"),
            }
        }
        Ok(())
    });
    if let Err(e) = filled {
        log::warn!("участники встреч не записались в индекс: {e}");
    }
}

#[cfg(test)]
mod tests {
    use nixyon_core::Speaker;

    use super::super::meeting_id::MeetingId;
    use super::super::participant::Participant;
    use super::*;

    #[test]
    fn meetings_of_the_previous_version_get_participants_from_their_transcripts() {
        let store = crate::vault::test_store("meetings-backfill");
        let mut t = super::super::test_transcript("2026-11-07T10:00:00Z");
        t.speakers = vec![Speaker {
            id: 2,
            name: None,
            talk_time_sec: 3.0,
            turns: 1,
            words: 1,
            age: None,
            gender: None,
        }];
        super::super::test_meeting(&store, &t);
        super::super::test_meeting(
            &store,
            &super::super::test_transcript("2026-11-07T11:00:00Z"),
        );
        let json = r#"{"meetings":[
            {"id":"2026-11-07T10:00:00Z","duration_sec":1.5,"file":"a.wav"},
            {"id":"2026-11-07T11:00:00Z","duration_sec":1.5,"file":"a.wav","title":"Своё"},
            {"id":"2026-11-07T12:00:00Z","duration_sec":1.5,"file":"нет.wav"}
        ]}"#;
        store.write(index::FILE, json.as_bytes()).unwrap();
        fill_participants(&store);
        let meetings = index::load(&store).unwrap();
        assert_eq!(
            meetings[0].participants,
            Some(vec![Participant { id: 2, name: None }])
        );
        assert_eq!(meetings[1].participants, Some(Vec::new()));
        assert_eq!(meetings[1].title.as_deref(), Some("Своё"));
        assert_eq!(meetings[2].participants, None);
        assert_eq!(
            meetings[2].id,
            MeetingId::parse("2026-11-07T12:00:00Z").unwrap()
        );
    }

    #[test]
    fn broken_index_is_left_untouched() {
        let store = crate::vault::test_store("meetings-backfill-broken");
        std::fs::write(store.dir().join(index::FILE), "{").unwrap();
        fill_participants(&store);
        assert_eq!(std::fs::read(store.dir().join(index::FILE)).unwrap(), b"{");
    }
}
