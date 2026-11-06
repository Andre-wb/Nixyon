//! Голоса участников в `files/voices.json`. Это личные данные: ни имена, ни эмбеддинги
//! не пишутся в лог.

use serde::{Deserialize, Serialize};

use super::voice_record::VoiceRecord;
use crate::vault::FilesStore;

const FILE: &str = "voices.json";

#[derive(Serialize, Deserialize)]
struct VoicesFile {
    voices: Vec<VoiceRecord>,
}

/// Нет файла: голосов нет. Битый файл: ошибка с видом `InvalidData`, чтобы не затереть
/// голоса при следующем сохранении.
pub fn load(store: &FilesStore) -> std::io::Result<Vec<VoiceRecord>> {
    match store.read(FILE)? {
        Some(bytes) => serde_json::from_slice::<VoicesFile>(&bytes)
            .map(|f| f.voices)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        None => Ok(Vec::new()),
    }
}

/// Оборванная запись не портит прежние голоса.
pub fn save(store: &FilesStore, voices: &[VoiceRecord]) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(&VoicesFile {
        voices: voices.to_vec(),
    })
    .map_err(std::io::Error::other)?;
    store.write(FILE, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> FilesStore {
        crate::vault::test_store(&format!("voices-{name}"))
    }

    fn voice(name: &str) -> VoiceRecord {
        VoiceRecord {
            name: name.into(),
            embedding: Some(vec![0.25, -0.5]),
            voice_model: Some("m.onnx".into()),
            created_at: "2026-10-29T10:00:00Z".into(),
            centers: Vec::new(),
        }
    }

    #[test]
    fn saved_voices_are_read_back_unchanged() {
        let d = dir("roundtrip");
        let voices = vec![voice("Анна"), voice("Борис")];
        save(&d, &voices).unwrap();
        assert_eq!(load(&d).unwrap(), voices);
        assert!(!d.dir().join("voices.json.part").exists());
    }

    #[test]
    fn missing_file_means_no_voices() {
        assert!(load(&dir("missing")).unwrap().is_empty());
    }

    #[test]
    fn broken_file_is_an_error_not_an_empty_list() {
        let d = dir("broken");
        std::fs::write(d.dir().join("voices.json"), "{\"voices\": [").unwrap();
        assert_eq!(
            load(&d).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}
