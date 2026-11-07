//! Кнопка «Удалить звук всех встреч» в разделе места на телефоне: звук всех встреч
//! архива удаляется, расшифровки и протоколы остаются.

use tauri::{AppHandle, Runtime};

use super::index;
use super::remove_audio::remove_audio;
use crate::transcription::CommandError;
use crate::vault::FilesStore;

/// Сколько встреч осталось без звука.
pub fn remove_all(store: &FilesStore) -> Result<usize, CommandError> {
    let meetings =
        index::load(store).map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
    let ids: Vec<_> = meetings.into_iter().map(|m| m.id).collect();
    Ok(remove_audio(store, &ids))
}

#[tauri::command]
pub async fn remove_all_audio<R: Runtime>(app: AppHandle<R>) -> Result<usize, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || remove_all(&store))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::audio_file;
    use super::super::meeting_dir::{AUDIO, MeetingDir};
    use super::super::{test_meeting, test_transcript, transcript_file};
    use super::*;

    #[test]
    fn audio_of_every_meeting_goes_and_transcripts_stay() {
        let store = crate::vault::test_store("remove-all-audio");
        let ids: Vec<_> = ["2026-11-01T10:00:00Z", "2026-11-07T10:00:00Z"]
            .into_iter()
            .map(|at| {
                let id = test_meeting(&store, &test_transcript(at));
                std::fs::write(MeetingDir::of(&id).path(store.dir()).join(AUDIO), b"RIFF").unwrap();
                id
            })
            .collect();
        assert_eq!(remove_all(&store).unwrap(), 2);
        for id in &ids {
            assert_eq!(audio_file::path(store.dir(), id), None);
            assert!(transcript_file::load(&store, id).is_some());
        }
    }
}
