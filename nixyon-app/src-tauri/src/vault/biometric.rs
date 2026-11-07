//! Вход по отпечатку в настройках: включён ли, переключение и экран добавления отпечатка
//! в Android. Включено ли, Kotlin определяет по обёртке ключа данных, флага в настройках
//! нет: файл настроек зашифрован этим же ключом и до разблокировки не читается.

use nixyon_android::{BiometricStatus, SpeechExt};
use tauri::{AppHandle, Runtime};

use super::storage_key_error;
use crate::transcription::CommandError;

/// `None` там, где отпечатка нет вообще (ноутбук): интерфейс тогда прячет настройку.
#[tauri::command]
pub async fn biometric_status<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<BiometricStatus>, CommandError> {
    match app.speech().biometric_status().await {
        Ok(status) => Ok(Some(status)),
        Err(nixyon_android::Error::Unsupported) => Ok(None),
        Err(e) => Err(storage_key_error(e)),
    }
}

/// Включает или выключает вход по отпечатку. Без отпечатка на телефоне включение
/// отвечает `biometric_missing`, отменённый запрос пальца отвечает `locked`.
#[tauri::command]
pub async fn set_biometric<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
) -> Result<(), CommandError> {
    app.speech()
        .set_biometric(enabled)
        .await
        .map_err(storage_key_error)
}

#[tauri::command]
pub async fn open_biometric_settings<R: Runtime>(app: AppHandle<R>) -> Result<(), CommandError> {
    app.speech()
        .open_biometric_settings()
        .await
        .map_err(storage_key_error)
}
