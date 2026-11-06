//! Путь к исходной записи последней расшифровки: по нему её проигрывают. Хранится рядом с
//! расшифровкой отдельным файлом, чтобы не менять формат расшифровки и экспорт.

use std::io::Write;
use std::path::{Path, PathBuf};

const FILE: &str = "last_audio.txt";

fn path(files_dir: &Path) -> PathBuf {
    files_dir.join(FILE)
}

/// Пишется рядом и переименовывается: оборванная запись не оставит битый путь.
pub fn save(files_dir: &Path, audio: &Path) -> std::io::Result<()> {
    let text = audio
        .to_str()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "путь не UTF-8"))?;
    let part = files_dir.join(format!("{FILE}.part"));
    let mut file = std::fs::File::create(&part)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&part, path(files_dir))
}

/// Нет файла: ничего страшного, записи для проигрывания просто нет.
pub fn clear(files_dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path(files_dir)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

pub fn load(files_dir: &Path) -> Option<String> {
    std::fs::read_to_string(path(files_dir))
        .ok()
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nixyon-audio-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saved_path_is_loaded_and_cleared() {
        let d = dir("roundtrip");
        assert_eq!(load(&d), None);
        clear(&d).unwrap();
        save(&d, Path::new("/data/rec/запись.wav")).unwrap();
        assert_eq!(load(&d).as_deref(), Some("/data/rec/запись.wav"));
        clear(&d).unwrap();
        assert_eq!(load(&d), None);
    }
}
