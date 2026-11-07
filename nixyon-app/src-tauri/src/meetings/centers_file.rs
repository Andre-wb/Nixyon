//! Центры голосов спикеров встречи: по ним голос запоминают в библиотеку. Это личные
//! данные: в лог не пишутся.

use nixyon_pipeline::SpeakerCenter;
use serde::{Deserialize, Serialize};

use super::meeting_dir::{CENTERS, MeetingDir};
use super::meeting_id::MeetingId;
use crate::vault::FilesStore;

#[derive(Serialize, Deserialize)]
struct CenterEntry {
    speaker: u32,
    embedding: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
struct CentersFile {
    centers: Vec<CenterEntry>,
}

fn name(id: &MeetingId) -> String {
    MeetingDir::of(id).file(CENTERS)
}

/// Оборванная запись не оставит битый файл.
pub fn save(store: &FilesStore, id: &MeetingId, centers: &[SpeakerCenter]) -> std::io::Result<()> {
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
    store.write(&name(id), &json)
}

pub fn clear(store: &FilesStore, id: &MeetingId) -> std::io::Result<()> {
    store.remove(&name(id))
}

/// Нет файла или он битый: центров нет, запомнить голос нельзя.
pub fn load(store: &FilesStore, id: &MeetingId) -> Option<Vec<SpeakerCenter>> {
    let bytes = match store.read(&name(id)) {
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

    fn store(name: &str) -> (FilesStore, MeetingId) {
        let store = crate::vault::test_store(&format!("centers-{name}"));
        let id = MeetingId::parse("2026-10-28T12:00:00Z").unwrap();
        std::fs::create_dir_all(MeetingDir::of(&id).path(store.dir())).unwrap();
        (store, id)
    }

    #[test]
    fn centers_are_saved_loaded_and_cleared() {
        let (d, id) = store("roundtrip");
        assert_eq!(load(&d, &id), None);
        let centers = vec![(1, vec![0.6, 0.8]), (3, vec![1.0, 0.0])];
        save(&d, &id, &centers).unwrap();
        assert_eq!(load(&d, &id), Some(centers));
        clear(&d, &id).unwrap();
        clear(&d, &id).unwrap();
        assert_eq!(load(&d, &id), None);
    }

    #[test]
    fn broken_file_gives_no_centers() {
        let (d, id) = store("broken");
        std::fs::write(d.dir().join(name(&id)), "{\"centers\": [").unwrap();
        assert_eq!(load(&d, &id), None);
    }
}
