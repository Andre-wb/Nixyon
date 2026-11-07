//! Статистика расшифровки встречи: считается по ходам при каждом открытии, в файл
//! расшифровки не пишется.

mod stats_view;

use nixyon_pipeline::meeting_stats as compute;
use tauri::{AppHandle, Runtime};

use crate::transcription::CommandError;
use crate::vault::FilesStore;
pub use stats_view::StatsView;

pub fn load(store: &FilesStore, meeting: &str) -> Result<StatsView, CommandError> {
    let (_, transcript) = crate::meetings::load(store, meeting)?;
    Ok(StatsView::from(&compute(&transcript)))
}

#[tauri::command]
pub async fn meeting_stats<R: Runtime>(
    app: AppHandle<R>,
    meeting: String,
) -> Result<StatsView, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || load(&store, &meeting))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use nixyon_core::{
        AudioInfo, EngineInfo, SCHEMA_VERSION, Speaker, SpeechLanguage, Transcript, Turn,
    };
    use nixyon_pipeline::MONOLOGUE_SEC;

    fn sample() -> Transcript {
        let speaker = |id| Speaker {
            id,
            name: None,
            talk_time_sec: 0.0,
            turns: 0,
            words: 0,
            age: None,
            gender: None,
        };
        let turn = |id, speaker, start: f64, end: f64| Turn {
            id,
            speaker,
            start,
            end,
            text: "раз два".into(),
            words: None,
            flags: Vec::new(),
            emotion: None,
        };
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: M.into(),
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
            language: SpeechLanguage::Ru,
            speakers: vec![speaker(1), speaker(2)],
            turns: vec![turn(1, 1, 0.0, 6.0), turn(2, 2, 5.0, 8.0)],
        }
    }

    const M: &str = "2026-10-29T10:00:00Z";

    fn dir(name: &str) -> crate::vault::FilesStore {
        crate::vault::test_store(&format!("stats-{name}"))
    }

    #[test]
    fn stats_come_from_the_meeting_and_unknown_one_is_an_error() {
        let d = dir("load");
        assert_eq!(
            load(&d, M).unwrap_err().code,
            crate::meetings::NO_TRANSCRIPT
        );
        crate::meetings::test_meeting(&d, &sample());
        let stats = load(&d, M).unwrap();
        assert_eq!(stats.interruptions, 1);
        assert_eq!(stats.talk_time_sec, 9.0);
        assert_eq!(stats.monologue_sec, MONOLOGUE_SEC);
        assert_eq!(stats.speakers[0].words_per_minute, Some(20.0));
        assert_eq!(stats.speakers[1].interruptions, 1);
    }

    #[test]
    fn view_is_serialized_in_camel_case() {
        let d = dir("json");
        crate::meetings::test_meeting(&d, &sample());
        let json = serde_json::to_value(load(&d, M).unwrap()).unwrap();
        assert!(json["speakers"][0]["wordsPerMinute"].is_number());
        assert!(json["monologueSec"].is_number());
        assert!(json["talkTimeSec"].is_number());
    }
}
