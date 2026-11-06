//! Центры голосов спикеров последней расшифровки: по ним голос запоминают в библиотеку.
//! Это личные данные: в лог не пишутся.

use nixyon_pipeline::SpeakerCenter;
use serde::{Deserialize, Serialize};

use crate::vault::FilesStore;

const FILE: &str = "last_centers.json";

#[derive(Serialize, Deserialize)]
struct CenterEntry {
    speaker: u32,
    embedding: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
struct CentersFile {
    centers: Vec<CenterEntry>,
}

/// Оборванная запись не оставит битый файл.
pub fn save(store: &FilesStore, centers: &[SpeakerCenter]) -> std::io::Result<()> {
    let file = CentersFile {
        centers: centers
            .iter()
            .map(|(speaker, embedding)| CenterEntry {
                speaker: *speaker,
                embedding: embedding.clone(),
            })
            .collect(),
    };
    let json = serde_json::to_vec(&file).map_err(std::io::Error::other)?;
    store.write(FILE, &json)
}

pub fn clear(store: &FilesStore) -> std::io::Result<()> {
    store.remove(FILE)
}

/// Нет файла или он битый: центров нет, запомнить голос нельзя.
pub fn load(store: &FilesStore) -> Option<Vec<SpeakerCenter>> {
    let bytes = match store.read(FILE) {
        Ok(bytes) => bytes?,
        Err(e) => {
            log::warn!("центры голосов не читаются: {e}");
            return None;
        }
    };
    match serde_json::from_slice::<CentersFile>(&bytes) {
        Ok(file) => Some(
            file.centers
                .into_iter()
                .map(|c| (c.speaker, c.embedding))
                .collect(),
        ),
        Err(e) => {
            log::warn!("центры голосов не читаются: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> FilesStore {
        crate::vault::test_store(&format!("centers-{name}"))
    }

    #[test]
    fn centers_are_saved_loaded_and_cleared() {
        let d = dir("roundtrip");
        assert_eq!(load(&d), None);
        let centers = vec![(1, vec![0.6, 0.8]), (3, vec![1.0, 0.0])];
        save(&d, &centers).unwrap();
        assert_eq!(load(&d), Some(centers));
        clear(&d).unwrap();
        clear(&d).unwrap();
        assert_eq!(load(&d), None);
    }

    #[test]
    fn broken_file_gives_no_centers() {
        let d = dir("broken");
        std::fs::write(d.dir().join(FILE), "{\"centers\": [").unwrap();
        assert_eq!(load(&d), None);
    }
}
