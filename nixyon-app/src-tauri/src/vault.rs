//! Шифрование файлов с личными данными на устройстве: исходной записи, расшифровки,
//! центров и библиотеки голосов, настроек с горячими словами. Ключ данных хранит
//! Kotlin обёрнутым ключом Android Keystore и отдаёт его ядру при первом обращении.

pub mod biometric;
mod data_key;
mod files_store;
mod format;
mod prepare;
mod sealed_file;
mod sealed_reader;
mod sealed_writer;
pub mod temporary_files;

use std::path::PathBuf;
use std::sync::Arc;

use nixyon_android::{SpeechExt, StoragePaths};
use tauri::async_runtime::Mutex;
use tauri::{AppHandle, Manager, Runtime};

use crate::transcription::CommandError;

pub use data_key::DataKey;
pub use files_store::FilesStore;
#[cfg(test)]
pub use files_store::test_store;
pub use prepare::RECORDINGS_DIR;
pub use sealed_file::open_read;

/// Ключ данных после первой подготовки хранилища; до неё `None`.
#[derive(Default)]
pub struct VaultState(Mutex<Option<Arc<DataKey>>>);

pub struct Storage {
    pub store: FilesStore,
    pub paths: StoragePaths,
}

/// Ключ не открыт или вход по отпечатку не переключён: палец не приложен или отменён, на
/// телефоне нет отпечатка или система стёрла ключ. Эти коды интерфейс показывает как есть;
/// остальное считается сбоем хранилища.
fn storage_key_error(e: nixyon_android::Error) -> CommandError {
    match e.rejected_code() {
        Some(
            code @ ("locked" | "biometric_unavailable" | "biometric_missing" | "storage_key_lost"),
        ) => CommandError::new(code),
        _ => CommandError::logged(CommandError::STORAGE_FAILED, e),
    }
}

/// Открывает хранилище: отпечатком, если вход по нему включён. Приложение вызывает её первой при запуске: пока она не
/// прошла, остальные команды не начинаются.
#[tauri::command]
pub async fn unlock_storage<R: Runtime>(app: AppHandle<R>) -> Result<(), CommandError> {
    storage(&app).await.map(|_| ())
}

/// Начать с чистого хранилища, когда система стёрла ключ под отпечатком: старые файлы уже
/// не прочитать, их удалит подготовка хранилища при следующем открытии. Плагин отказывает,
/// если ключ не стёрт.
#[tauri::command]
pub async fn reset_storage<R: Runtime>(app: AppHandle<R>) -> Result<(), CommandError> {
    app.speech()
        .reset_storage_key()
        .await
        .map_err(storage_key_error)
}

/// Папки приложения и хранилище с ключом. Первый вызов после запуска получает ключ и
/// готовит хранилище; остальные ждут его, поэтому ни одна команда не тронет файлы раньше.
pub async fn storage<R: Runtime>(app: &AppHandle<R>) -> Result<Storage, CommandError> {
    let paths = app
        .speech()
        .storage_paths()
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
    let state = app.state::<VaultState>();
    let mut slot = state.0.lock().await;
    let key = match slot.as_ref() {
        Some(key) => Arc::clone(key),
        None => {
            let raw = app
                .speech()
                .storage_key()
                .await
                .map_err(storage_key_error)?;
            let key = Arc::new(DataKey::from_bytes(&raw.key).ok_or_else(|| {
                CommandError::logged(CommandError::STORAGE_FAILED, "ключ данных не той длины")
            })?);
            let store = FilesStore::new(PathBuf::from(&paths.files), Arc::clone(&key));
            let cache = PathBuf::from(&paths.cache);
            tauri::async_runtime::spawn_blocking(move || prepare::prepare(&store, &cache))
                .await
                .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
            *slot = Some(Arc::clone(&key));
            key
        }
    };
    Ok(Storage {
        store: FilesStore::new(PathBuf::from(&paths.files), key),
        paths,
    })
}
