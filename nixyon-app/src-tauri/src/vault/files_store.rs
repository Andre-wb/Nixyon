use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::data_key::DataKey;
use super::sealed_file;

/// Папка `files` приложения с ключом данных: файлы с личными данными читаются и
/// пишутся через неё и на диске всегда зашифрованы.
#[derive(Clone)]
pub struct FilesStore {
    dir: PathBuf,
    key: Arc<DataKey>,
}

impl FilesStore {
    pub fn new(dir: PathBuf, key: Arc<DataKey>) -> Self {
        Self { dir, key }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn key(&self) -> &Arc<DataKey> {
        &self.key
    }

    /// Нет файла: `None`.
    pub fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        match sealed_file::read_bytes(&self.dir.join(name), &self.key) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn write(&self, name: &str, data: &[u8]) -> std::io::Result<()> {
        sealed_file::write_atomic(&self.dir.join(name), &self.key, data)
    }

    /// Нет файла: ничего страшного.
    pub fn remove(&self, name: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.dir.join(name)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
pub fn test_store(name: &str) -> FilesStore {
    let dir = std::env::temp_dir().join(format!("nixyon-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    FilesStore::new(dir, Arc::new(super::data_key::test_key(9)))
}
