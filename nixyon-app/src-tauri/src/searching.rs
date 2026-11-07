//! Поиск по текстам реплик последней расшифровки. Запрос в лог не пишется: в нём могут
//! быть слова из разговора.

mod hit_view;

use nixyon_pipeline::search;
use tauri::{AppHandle, Runtime};

use crate::transcription::{CommandError, last_transcript};
use crate::vault::FilesStore;
pub use hit_view::HitView;

const NO_TRANSCRIPT: &str = "no_transcript";

pub fn find(store: &FilesStore, query: &str) -> Result<Vec<HitView>, CommandError> {
    let transcript =
        last_transcript::load(store).ok_or_else(|| CommandError::new(NO_TRANSCRIPT))?;
    Ok(search(&transcript, query)
        .iter()
        .map(HitView::from)
        .collect())
}

#[tauri::command]
pub async fn search_transcript<R: Runtime>(
    app: AppHandle<R>,
    query: String,
) -> Result<Vec<HitView>, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || find(&store, &query))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION, Speaker, Transcript, Turn};

    fn sample() -> Transcript {
        let turn = |id, text: &str| Turn {
            id,
            speaker: 1,
            start: f64::from(id),
            end: f64::from(id) + 1.0,
            text: text.into(),
            words: None,
            flags: Vec::new(),
            emotion: None,
        };
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: "2026-10-29T10:00:00Z".into(),
            audio: AudioInfo {
                file: "a.wav".into(),
                duration_sec: 10.0,
                sample_rate: 16_000,
            },
            engine: EngineInfo {
                vad: "v".into(),
                asr: "a".into(),
                embedding: "e".into(),
            },
            speakers: vec![Speaker {
                id: 1,
                name: None,
                talk_time_sec: 0.0,
                turns: 0,
                words: 0,
                age: None,
                gender: None,
            }],
            turns: vec![turn(1, "Встреча завтра."), turn(2, "Про встерчу забыли")],
        }
    }

    fn dir(name: &str) -> crate::vault::FilesStore {
        crate::vault::test_store(&format!("search-{name}"))
    }

    #[test]
    fn hits_come_from_last_transcript_and_missing_one_is_an_error() {
        let d = dir("find");
        assert_eq!(find(&d, "встреча").unwrap_err().code, NO_TRANSCRIPT);
        last_transcript::save(&d, &sample()).unwrap();
        let hits = find(&d, "встреча").unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(
            (hits[0].turn_id, hits[0].start, hits[0].end, hits[0].exact),
            (1, 0, 7, true)
        );
        assert_eq!((hits[1].turn_id, hits[1].exact), (2, false));
        let json = serde_json::to_value(&hits[0]).unwrap();
        assert!(json["turnId"].is_number());
    }
}
