use nixyon_core::{Transcript, from_json, to_json};

use crate::vault::FilesStore;

const FILE: &str = "last_transcript.json";

/// Новая расшифровка заменяет прошлую целиком или не заменяет вовсе: оборванная запись
/// не оставит битый файл.
pub fn save(store: &FilesStore, transcript: &Transcript) -> std::io::Result<()> {
    let json = to_json(transcript).map_err(std::io::Error::other)?;
    store.write(FILE, json.as_bytes())
}

/// Последняя расшифровка, если она есть и читается.
pub fn load(store: &FilesStore) -> Option<Transcript> {
    let bytes = match store.read(FILE) {
        Ok(bytes) => bytes?,
        Err(e) => {
            log::error!("последняя расшифровка не читается: {e}");
            return None;
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => {
            log::error!("последняя расшифровка не читается: {e}");
            return None;
        }
    };
    match from_json(&text) {
        Ok(t) => Some(t),
        Err(e) => {
            log::error!("последняя расшифровка не читается: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION};

    fn transcript(file: &str) -> Transcript {
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: "2026-10-28T12:00:00Z".into(),
            audio: AudioInfo {
                file: file.into(),
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

    #[test]
    fn new_transcript_replaces_the_previous_one() {
        let store = crate::vault::test_store("last-transcript");
        assert!(load(&store).is_none());
        save(&store, &transcript("a.wav")).unwrap();
        save(&store, &transcript("b.wav")).unwrap();
        assert_eq!(load(&store).unwrap().audio.file, "b.wav");
        assert!(!store.dir().join("last_transcript.json.part").exists());
        let on_disk = std::fs::read(store.dir().join(FILE)).unwrap();
        assert!(!on_disk.windows(5).any(|w| w == b"b.wav"));
        std::fs::write(store.dir().join(FILE), "{").unwrap();
        assert!(load(&store).is_none());
    }
}
