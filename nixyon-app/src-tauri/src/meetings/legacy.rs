//! Файлы единственной последней встречи от прежних версий. Архив начинается с нуля: они
//! удаляются при запуске, в архив не переносятся.

use std::path::Path;

const LEGACY: [&str; 4] = [
    "last_transcript.json",
    "last_summary.json",
    "last_centers.json",
    "last_audio.txt",
];

pub fn remove(files_dir: &Path) {
    for name in LEGACY {
        for file in [name.to_owned(), format!("{name}.part")] {
            if let Err(e) = std::fs::remove_file(files_dir.join(&file))
                && e.kind() != std::io::ErrorKind::NotFound
            {
                log::warn!("старый файл {file} не удалён: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_of_the_single_last_meeting_are_removed() {
        let store = crate::vault::test_store("meetings-legacy");
        for name in LEGACY {
            std::fs::write(store.dir().join(name), b"x").unwrap();
        }
        std::fs::write(store.dir().join("last_transcript.json.part"), b"x").unwrap();
        std::fs::write(store.dir().join("voices.json"), b"x").unwrap();
        remove(store.dir());
        remove(store.dir());
        for name in LEGACY {
            assert!(!store.dir().join(name).exists(), "{name}");
        }
        assert!(!store.dir().join("last_transcript.json.part").exists());
        assert!(store.dir().join("voices.json").exists());
    }
}
