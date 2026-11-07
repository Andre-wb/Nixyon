//! Раздел «Место на телефоне» в настройках: сколько занимают записи, расшифровки, модели,
//! голоса и временные файлы, и очистка временных файлов.

mod disk_size;
mod measure;
mod usage;

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Runtime, State};

use crate::summary::SummaryRunning;
use crate::transcription::{CommandError, Running};
use usage::StorageUsage;

#[tauri::command]
pub async fn storage_usage<R: Runtime>(app: AppHandle<R>) -> Result<StorageUsage, CommandError> {
    let paths = crate::vault::storage(&app).await?.paths;
    tauri::async_runtime::spawn_blocking(move || {
        measure::measure(
            Path::new(&paths.files),
            Path::new(&paths.cache),
            paths.external_files.as_deref().map(Path::new),
        )
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
}

/// Пока идёт расшифровка, пересчёт реплики или работа языковой модели, их временные
/// файлы нужны: очистка отвечает `busy` и ничего не трогает.
#[tauri::command]
pub async fn clear_temporary_files<R: Runtime>(
    app: AppHandle<R>,
    listening: State<'_, Running>,
    checking: State<'_, SummaryRunning>,
) -> Result<(), CommandError> {
    let _listen = listening
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;
    let _check = checking
        .0
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;
    let paths = crate::vault::storage(&app).await?.paths;
    let (files, cache) = (PathBuf::from(paths.files), PathBuf::from(paths.cache));
    tauri::async_runtime::spawn_blocking(move || {
        crate::vault::temporary_files::remove(&files, &cache);
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
}
