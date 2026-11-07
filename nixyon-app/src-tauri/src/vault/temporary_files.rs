//! Временные файлы приложения: звук и разметка на время расшифровки (`cache/nixyon-*`),
//! файлы экспорта и записи образцов голоса. Их убирает запуск, кнопка «Очистить временные
//! файлы» и подсчёт места берут тот же список. Остальное в `cache` (например, кэш
//! WebView) не наше, его не трогаем.

use std::path::{Path, PathBuf};

/// Папка записей образцов голоса (`SamplesDir.kt`): образец нужен только до сохранения
/// голоса, после перезапуска он уже не нужен.
pub const SAMPLES_DIR: &str = "samples";
/// Папка файлов экспорта (`exporting.rs`): файл нужен, только пока его сохраняют или
/// отправляют; открытым до следующего экспорта он лежать не должен.
pub const EXPORTS_DIR: &str = "exports";
const TEMP_PREFIX: &str = "nixyon-";

pub fn list(files_dir: &Path, cache: &Path) -> Vec<PathBuf> {
    let mut found = files_in(cache, |name| name.starts_with(TEMP_PREFIX));
    found.extend(files_in(&cache.join(EXPORTS_DIR), |_| true));
    found.extend(files_in(&files_dir.join(SAMPLES_DIR), |_| true));
    found
}

pub fn remove(files_dir: &Path, cache: &Path) {
    for path in list(files_dir, cache) {
        if let Err(e) = std::fs::remove_file(&path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("временный файл не удалён: {e}");
        }
    }
}

fn files_in(dir: &Path, matches: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
        .filter(|entry| matches(&entry.file_name().to_string_lossy()))
        .map(|entry| entry.path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_our_temporary_files_are_listed_and_removed() {
        let store = crate::vault::test_store("temporary-files");
        let (files, cache) = (store.dir().to_path_buf(), store.dir().join("cache"));
        std::fs::create_dir_all(cache.join(EXPORTS_DIR)).unwrap();
        std::fs::create_dir_all(files.join(SAMPLES_DIR)).unwrap();
        std::fs::create_dir_all(cache.join("WebView")).unwrap();
        for path in [
            cache.join("nixyon-1.wav"),
            cache.join("exports/встреча.txt"),
            files.join("samples/smp-1.wav"),
            cache.join("other.bin"),
            cache.join("WebView/nixyon-2.wav"),
            files.join("voices.json"),
        ] {
            std::fs::write(path, b"x").unwrap();
        }
        let mut found = list(&files, &cache);
        found.sort();
        let mut expected = vec![
            cache.join("nixyon-1.wav"),
            cache.join("exports/встреча.txt"),
            files.join("samples/smp-1.wav"),
        ];
        expected.sort();
        assert_eq!(found, expected);
        remove(&files, &cache);
        assert!(list(&files, &cache).is_empty());
        assert!(cache.join("other.bin").exists());
        assert!(cache.join("WebView/nixyon-2.wav").exists());
        assert!(files.join("voices.json").exists());
    }
}
