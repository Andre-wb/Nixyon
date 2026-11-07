//! Подготовка хранилища при первом обращении после запуска, до любой работы с файлами:
//! остатки временных файлов удаляются, открытые файлы от прежних версий шифруются,
//! файлы под потерянным ключом удаляются, архив встреч приводится в порядок. Имена файлов
//! записей не личные, их можно в лог.

use std::path::Path;

use super::files_store::FilesStore;
use super::sealed_file::{first_chunk_opens, is_sealed, seal_in_place};

/// Файлы с личными данными в `files`; файлы встреч лежат в своих папках.
pub const PRIVATE_FILES: [&str; 3] = [crate::meetings::INDEX_FILE, "voices.json", "settings.json"];
/// Папка исходных записей Kotlin-плагина (`RecordingsDir.kt`).
pub const RECORDINGS_DIR: &str = "recordings";
/// Папка записей образцов голоса (`SamplesDir.kt`): образец нужен только до сохранения
/// голоса, после перезапуска он уже не нужен.
pub const SAMPLES_DIR: &str = "samples";
/// Папка файлов экспорта (`exporting.rs`): файл нужен, только пока его сохраняют или
/// отправляют; открытым до следующего экспорта он лежать не должен.
pub const EXPORTS_DIR: &str = "exports";
const TEMP_PREFIX: &str = "nixyon-";

pub fn prepare(store: &FilesStore, cache: &Path) {
    sweep(cache, |name| name.starts_with(TEMP_PREFIX));
    sweep(&cache.join(EXPORTS_DIR), |_| true);
    sweep(&store.dir().join(SAMPLES_DIR), |_| true);
    for name in PRIVATE_FILES {
        remove_quietly(&store.dir().join(format!("{name}.part")));
        secure(store, &store.dir().join(name));
    }
    // Индекс уже проверен выше: под потерянным ключом он удалён, и папки встреч, тоже
    // нечитаемые, уходят вместе с ним.
    crate::meetings::startup::tidy(store, &|path| secure(store, path));
    crate::meetings::backfill::fill_participants(store);
    let recordings = store.dir().join(RECORDINGS_DIR);
    sweep(&recordings, |name| name.ends_with(".part"));
    if let Ok(entries) = std::fs::read_dir(&recordings) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_file()) {
                secure(store, &entry.path());
            }
        }
    }
}

fn secure(store: &FilesStore, path: &Path) {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    match is_sealed(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::error!("файл {name} не читается: {e}"),
        Ok(true) => match first_chunk_opens(path, store.key()) {
            Ok(true) => {}
            Ok(false) => {
                log::warn!("файл {name} не расшифровывается этим ключом, удаляю");
                remove_quietly(path);
            }
            Err(e) => log::error!("файл {name} не проверен: {e}"),
        },
        Ok(false) => match seal_in_place(path, store.key()) {
            Ok(()) => log::info!("файл {name} зашифрован"),
            Err(e) => log::error!("файл {name} не зашифрован: {e}"),
        },
    }
}

fn sweep(dir: &Path, matches: impl Fn(&str) -> bool) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let is_file = entry.file_type().is_ok_and(|t| t.is_file());
        if is_file && matches(&entry.file_name().to_string_lossy()) {
            remove_quietly(&entry.path());
        }
    }
}

fn remove_quietly(path: &Path) {
    if let Err(e) = std::fs::remove_file(path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("файл не удалён: {e}");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::data_key::test_key;
    use super::super::files_store::test_store;
    use super::super::sealed_file::{read_bytes, write_atomic};
    use super::*;

    #[test]
    fn plain_files_get_sealed_leftovers_go_and_files_under_a_lost_key_are_removed() {
        let store = test_store("prepare");
        let cache = store.dir().join("cache");
        let rec = store.dir().join(RECORDINGS_DIR);
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir_all(cache.join("exports")).unwrap();
        std::fs::write(cache.join("exports/ru2.txt"), b"plain transcript").unwrap();
        std::fs::create_dir_all(store.dir().join(SAMPLES_DIR)).unwrap();
        std::fs::write(store.dir().join("samples/smp-1.wav"), b"sample").unwrap();
        std::fs::create_dir_all(&rec).unwrap();
        std::fs::write(cache.join("nixyon-1.wav"), b"pcm").unwrap();
        std::fs::write(cache.join("nixyon-1.segments.json"), b"[]").unwrap();
        std::fs::write(cache.join("other.bin"), b"x").unwrap();
        std::fs::write(store.dir().join("meetings.json"), b"{\"meetings\":[]}").unwrap();
        std::fs::write(store.dir().join("last_transcript.json"), b"{\"t\":1}").unwrap();
        std::fs::write(store.dir().join("voices.json.part"), b"{").unwrap();
        std::fs::write(rec.join("rec-1.wav"), b"RIFF-plain").unwrap();
        std::fs::write(rec.join("rec-2.wav.part"), b"RIFF").unwrap();
        write_atomic(
            &store.dir().join("settings.json"),
            &Arc::new(test_key(7)),
            b"{}",
        )
        .unwrap();
        write_atomic(&store.dir().join("voices.json"), store.key(), b"{\"v\":[]}").unwrap();

        prepare(&store, &cache);

        assert!(!cache.join("nixyon-1.wav").exists());
        assert!(!cache.join("nixyon-1.segments.json").exists());
        assert!(cache.join("other.bin").exists());
        assert!(cache.join("exports").is_dir());
        assert!(!cache.join("exports/ru2.txt").exists());
        assert!(!store.dir().join("samples/smp-1.wav").exists());
        assert!(!store.dir().join("voices.json.part").exists());
        assert!(!rec.join("rec-2.wav.part").exists());
        for (path, plain) in [
            (store.dir().join("meetings.json"), &b"{\"meetings\":[]}"[..]),
            (rec.join("rec-1.wav"), &b"RIFF-plain"[..]),
            (store.dir().join("voices.json"), &b"{\"v\":[]}"[..]),
        ] {
            assert!(is_sealed(&path).unwrap(), "{}", path.display());
            assert_eq!(read_bytes(&path, store.key()).unwrap(), plain);
        }
        assert!(!store.dir().join("settings.json").exists());
        assert!(!store.dir().join("last_transcript.json").exists());
    }

    #[test]
    fn plain_meeting_files_get_sealed_and_a_meeting_under_a_lost_key_goes_with_its_index() {
        let store = test_store("prepare-meetings");
        let cache = store.dir().join("cache");
        let dir = store.dir().join("meetings/20261107T100000Z");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("transcript.json"), b"{\"t\":1}").unwrap();
        let index =
            br#"{"meetings":[{"id":"2026-11-07T10:00:00Z","duration_sec":1.0,"file":"a.wav"}]}"#;
        write_atomic(&store.dir().join("meetings.json"), store.key(), index).unwrap();

        prepare(&store, &cache);

        assert!(is_sealed(&dir.join("transcript.json")).unwrap());
        assert_eq!(
            read_bytes(&dir.join("transcript.json"), store.key()).unwrap(),
            b"{\"t\":1}"
        );

        write_atomic(
            &store.dir().join("meetings.json"),
            &Arc::new(test_key(7)),
            index,
        )
        .unwrap();
        prepare(&store, &cache);
        assert!(!store.dir().join("meetings.json").exists());
        assert!(!dir.exists());
    }
}
