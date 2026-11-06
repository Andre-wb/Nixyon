//! Протокол последней расшифровки, составленный моделью. Личные данные: файл зашифрован,
//! в лог текст не пишется. Протокол привязан к расшифровке по времени её создания:
//! протокол от другой расшифровки не показывается.

use nixyon_core::Transcript;
use serde::{Deserialize, Serialize};

use crate::vault::FilesStore;

const FILE: &str = "last_summary.json";

#[derive(Serialize, Deserialize)]
struct SummaryFile {
    transcript_created_at: String,
    text: String,
}

pub fn save(store: &FilesStore, transcript: &Transcript, text: &str) -> std::io::Result<()> {
    let file = SummaryFile {
        transcript_created_at: transcript.created_at.clone(),
        text: text.to_owned(),
    };
    let json = serde_json::to_vec(&file).map_err(std::io::Error::other)?;
    store.write(FILE, &json)
}

/// Протокол этой расшифровки, если он есть и читается.
pub fn load(store: &FilesStore, transcript: &Transcript) -> Option<String> {
    let bytes = match store.read(FILE) {
        Ok(bytes) => bytes?,
        Err(e) => {
            log::error!("протокол встречи не читается: {e}");
            return None;
        }
    };
    match serde_json::from_slice::<SummaryFile>(&bytes) {
        Ok(file) if file.transcript_created_at == transcript.created_at => Some(file.text),
        Ok(_) => None,
        Err(e) => {
            log::error!("протокол встречи не читается: {e}");
            None
        }
    }
}

pub fn clear(store: &FilesStore) -> std::io::Result<()> {
    store.remove(FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION};

    fn transcript(created_at: &str) -> Transcript {
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: created_at.into(),
            audio: AudioInfo {
                file: "a.wav".into(),
                duration_sec: 1.0,
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

    #[test]
    fn summary_is_shown_only_for_the_transcript_it_was_made_from() {
        let store = crate::vault::test_store("last-summary");
        let first = transcript("2026-10-30T10:00:00Z");
        let second = transcript("2026-10-30T11:00:00Z");
        assert!(load(&store, &first).is_none());
        save(&store, &first, "Задачи: - Анна: позвонить").unwrap();
        assert_eq!(
            load(&store, &first).as_deref(),
            Some("Задачи: - Анна: позвонить")
        );
        assert!(load(&store, &second).is_none());
        let on_disk = std::fs::read(store.dir().join(FILE)).unwrap();
        assert!(
            !on_disk
                .windows("Анна".len())
                .any(|w| w == "Анна".as_bytes())
        );
        clear(&store).unwrap();
        assert!(load(&store, &first).is_none());
        std::fs::write(store.dir().join(FILE), "{").unwrap();
        assert!(load(&store, &first).is_none());
    }
}
