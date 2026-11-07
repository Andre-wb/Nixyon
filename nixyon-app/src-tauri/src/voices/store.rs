//! Голоса участников в `files/voices.json`. Это личные данные: ни имена, ни эмбеддинги
//! не пишутся в лог.

use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use super::voice_record::VoiceRecord;
use crate::vault::FilesStore;

const FILE: &str = "voices.json";

/// Голоса меняют добавление образца, удаление и «запомнить голос», и команды могут прийти
/// одновременно. Без общей блокировки на весь цикл «прочитать, изменить, записать» одно из
/// изменений потерялось бы.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

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

/// Читает голоса, даёт их изменить и записывает. Внешняя ошибка: файл не прочитался или
/// не записался. Внутренняя: изменение отказало, и файл остался как был.
pub fn update<T, E>(
    store: &FilesStore,
    change: impl FnOnce(&mut Vec<VoiceRecord>) -> Result<T, E>,
) -> std::io::Result<Result<T, E>> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let mut voices = load(store)?;
    let result = change(&mut voices);
    if result.is_ok() {
        save(store, &voices)?;
    }
    Ok(result)
}

/// Оборванная запись не портит прежние голоса.
fn save(store: &FilesStore, voices: &[VoiceRecord]) -> std::io::Result<()> {
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
    fn refused_change_leaves_the_voices_as_they_were() {
        let d = dir("refused");
        save(&d, &[voice("Анна")]).unwrap();
        let refused = update(&d, |v| {
            v.clear();
            Err::<(), _>("отказ")
        })
        .unwrap();
        assert_eq!(refused, Err("отказ"));
        assert_eq!(load(&d).unwrap(), vec![voice("Анна")]);
    }

    #[test]
    fn simultaneous_changes_lose_no_voice() {
        let d = dir("race");
        std::thread::scope(|s| {
            for n in 0..16 {
                let d = &d;
                s.spawn(move || {
                    update(d, |v| {
                        // Пауза между чтением и записью: без блокировки все потоки успели
                        // бы прочитать один и тот же список.
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        v.push(voice(&format!("Голос {n}")));
                        Ok::<_, ()>(())
                    })
                    .unwrap()
                    .unwrap()
                });
            }
        });
        assert_eq!(load(&d).unwrap().len(), 16);
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
