//! Запись правленой расшифровки встречи. Имена и состав спикеров живут в расшифровке, а
//! список встреч показывает участников из индекса, поэтому после записи обновляется и
//! строка индекса. Все правки расшифровки идут через эту функцию.

use nixyon_core::Transcript;

use super::index;
use super::meeting_id::MeetingId;
use super::participant::participants;
use super::transcript_file;
use crate::vault::FilesStore;

/// Ошибка только у самой расшифровки. Если не записался индекс, правка остаётся в силе:
/// причина уходит в лог, участники в списке обновятся при следующей правке.
pub fn save_transcript(
    store: &FilesStore,
    id: &MeetingId,
    transcript: &Transcript,
) -> std::io::Result<()> {
    transcript_file::save(store, id, transcript)?;
    let updated = index::update(store, |meetings| {
        let entry = meetings
            .iter_mut()
            .find(|m| &m.id == id)
            .ok_or_else(index::not_found)?;
        entry.participants = Some(participants(transcript));
        Ok(())
    });
    if let Err(e) = updated {
        log::warn!("участники встречи в индексе не обновились: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use nixyon_core::Speaker;

    use super::super::participant::Participant;
    use super::*;

    #[test]
    fn edited_transcript_refreshes_participants_and_keeps_the_title() {
        let store = crate::vault::test_store("save-transcript");
        let mut t = super::super::test_transcript("2026-11-07T10:00:00Z");
        let id = super::super::test_meeting(&store, &t);
        index::update(&store, |m| {
            m[0].title = Some("Планёрка".into());
            Ok(())
        })
        .unwrap();
        t.speakers = vec![Speaker {
            id: 1,
            name: Some("Анна".into()),
            talk_time_sec: 1.0,
            turns: 1,
            words: 1,
            age: None,
            gender: None,
        }];
        save_transcript(&store, &id, &t).unwrap();
        assert_eq!(transcript_file::load(&store, &id), Some(t));
        let entry = index::find(&store, &id).unwrap().unwrap();
        assert_eq!(entry.title.as_deref(), Some("Планёрка"));
        assert_eq!(
            entry.participants,
            Some(vec![Participant {
                id: 1,
                name: Some("Анна".into())
            }])
        );
    }
}
