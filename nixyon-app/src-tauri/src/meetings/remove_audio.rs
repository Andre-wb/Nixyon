//! Удаление исходной записи у встреч: расшифровка, протокол и центры остаются. Идёт под
//! блокировкой правок, как удаление встречи: пока звук стирается, встречу не удалят и
//! не перезапишут. Встреча, чей звук сейчас в работе, пропускается.

use super::audio_file;
use super::audio_in_use::is_in_use;
use super::edit_lock;
use super::meeting_id::MeetingId;
use crate::vault::FilesStore;

/// Сколько встреч осталось без звука после этого вызова. Не удалённый файл не мешает
/// остальным: причина в логе, звук удалит следующая очистка.
pub fn remove_audio(store: &FilesStore, ids: &[MeetingId]) -> usize {
    edit_lock::locked(|| {
        ids.iter()
            .filter(|id| !is_in_use(id))
            .filter_map(|id| audio_file::path(store.dir(), id))
            .filter(|path| match std::fs::remove_file(path) {
                Ok(()) => true,
                Err(e) => {
                    log::warn!("звук встречи не удалён: {e}");
                    false
                }
            })
            .count()
    })
}

#[cfg(test)]
mod tests {
    use super::super::audio_in_use::AudioInUse;
    use super::super::meeting_dir::{AUDIO, MeetingDir};
    use super::super::{summary_file, test_meeting, test_transcript, transcript_file};
    use super::*;

    fn with_audio(store: &FilesStore, created_at: &str) -> MeetingId {
        let id = test_meeting(store, &test_transcript(created_at));
        summary_file::save(store, &id, "Итоги").unwrap();
        std::fs::write(MeetingDir::of(&id).path(store.dir()).join(AUDIO), b"RIFF").unwrap();
        id
    }

    #[test]
    fn audio_goes_and_transcript_and_summary_stay() {
        let store = crate::vault::test_store("remove-audio");
        let gone = with_audio(&store, "2026-11-01T10:00:00Z");
        let kept = with_audio(&store, "2026-11-07T10:00:00Z");
        assert_eq!(remove_audio(&store, std::slice::from_ref(&gone)), 1);
        assert_eq!(audio_file::path(store.dir(), &gone), None);
        assert!(transcript_file::load(&store, &gone).is_some());
        assert_eq!(summary_file::load(&store, &gone).as_deref(), Some("Итоги"));
        assert!(audio_file::path(store.dir(), &kept).is_some());
        // Повтор: звука уже нет, удалять нечего.
        assert_eq!(remove_audio(&store, &[gone]), 0);
    }

    #[test]
    fn meeting_whose_audio_is_being_reprocessed_keeps_it() {
        let store = crate::vault::test_store("remove-audio-in-use");
        // Метка занятости одна на процесс: id этой встречи не встречается в других тестах,
        // иначе они, идущие одновременно, увидели бы её звук занятым.
        let busy = with_audio(&store, "1999-12-31T23:59:58Z");
        let free = with_audio(&store, "1999-12-31T23:59:59Z");
        let held = AudioInUse::hold(busy.clone());
        assert_eq!(remove_audio(&store, &[busy.clone(), free.clone()]), 1);
        assert!(audio_file::path(store.dir(), &busy).is_some());
        assert_eq!(audio_file::path(store.dir(), &free), None);
        drop(held);
        assert_eq!(remove_audio(&store, &[busy]), 1);
    }
}
