use std::path::PathBuf;

/// Временные файлы обработки удаляются при выходе из команды при любом исходе:
/// успехе, ошибке или отмене.
pub struct TempFiles(Vec<PathBuf>);

impl TempFiles {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        Self(paths)
    }
}

impl Drop for TempFiles {
    fn drop(&mut self) {
        for path in &self.0 {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => log::warn!("временный файл не удалён: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_files_are_removed_and_missing_ones_are_fine() {
        let dir = std::env::temp_dir().join(format!("nixyon-temp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.wav");
        std::fs::write(&a, b"x").unwrap();
        drop(TempFiles::new(vec![a.clone(), dir.join("нет.json")]));
        assert!(!a.exists());
    }
}
