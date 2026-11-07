//! Настройки приложения: язык интерфейса, тема, горячие слова для распознавания и
//! показан ли экран загрузки моделей.
//! Хранятся в `files/settings.json`.

mod hotword_list;

use nixyon_android::{SpeechExt, SystemBarsRequest};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::transcription::CommandError;
use crate::vault::FilesStore;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    /// Язык устройства; неизвестный язык даёт русский. Выбирает интерфейс.
    #[default]
    Auto,
    Ru,
    En,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Как в системе. Выбирает интерфейс.
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub theme: Theme,
    /// Слова и имена, которые распознавание пишет охотнее; по одному на элемент.
    #[serde(default)]
    pub hotwords: Vec<String>,
    /// Экран загрузки моделей уже показан: дальше он открывается только по кнопке.
    #[serde(default)]
    pub models_prompt_seen: bool,
}

pub use hotword_list::merge_hotwords;

const FILE: &str = "settings.json";

/// Битый или отсутствующий файл даёт настройки по умолчанию: язык не должен мешать
/// запуску приложения.
pub fn load(store: &FilesStore) -> Settings {
    let bytes = match store.read(FILE) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Settings::default(),
        Err(e) => {
            log::warn!("настройки не читаются, беру по умолчанию: {e}");
            return Settings::default();
        }
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        log::warn!("настройки не читаются, беру по умолчанию: {e}");
        Settings::default()
    })
}

pub fn save(store: &FilesStore, settings: &Settings) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?;
    store.write(FILE, &json)
}

async fn files_store<R: Runtime>(app: &AppHandle<R>) -> Result<FilesStore, CommandError> {
    Ok(crate::vault::storage(app).await?.store)
}

#[tauri::command]
pub async fn get_settings<R: Runtime>(app: AppHandle<R>) -> Result<Settings, CommandError> {
    let store = files_store(&app).await?;
    tauri::async_runtime::spawn_blocking(move || load(&store))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
}

#[tauri::command]
pub async fn set_language<R: Runtime>(
    app: AppHandle<R>,
    language: Language,
) -> Result<Settings, CommandError> {
    let store = files_store(&app).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut settings = load(&store);
        settings.language = language;
        save(&store, &settings)
            .map(|()| settings)
            .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[tauri::command]
pub async fn set_theme<R: Runtime>(
    app: AppHandle<R>,
    theme: Theme,
) -> Result<Settings, CommandError> {
    let store = files_store(&app).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut settings = load(&store);
        settings.theme = theme;
        save(&store, &settings)
            .map(|()| settings)
            .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

/// Значки строки состояния и навигации под тему интерфейса. Тему выбирает интерфейс:
/// при «Авто» он знает, тёмная ли система.
#[tauri::command]
pub async fn set_system_bars<R: Runtime>(
    app: AppHandle<R>,
    dark: bool,
) -> Result<(), CommandError> {
    app.speech()
        .set_system_bars(SystemBarsRequest { dark })
        .await
        .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))
}

/// Сохраняет список горячих слов из текста по одному в строке.
#[tauri::command]
pub async fn set_hotwords<R: Runtime>(
    app: AppHandle<R>,
    text: String,
) -> Result<Settings, CommandError> {
    let words = hotword_list::parse_hotwords(&text).map_err(|e| {
        CommandError::new(match e {
            hotword_list::HotwordListError::TooLong => CommandError::HOTWORD_TOO_LONG,
            hotword_list::HotwordListError::TooMany => CommandError::TOO_MANY_HOTWORDS,
        })
    })?;
    let store = files_store(&app).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut settings = load(&store);
        settings.hotwords = words;
        save(&store, &settings)
            .map(|()| settings)
            .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

/// Отмечает, что экран загрузки моделей показан: при следующем запуске он сам не откроется.
#[tauri::command]
pub async fn set_models_prompt_seen<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Settings, CommandError> {
    let store = files_store(&app).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut settings = load(&store);
        settings.models_prompt_seen = true;
        save(&store, &settings)
            .map(|()| settings)
            .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_and_theme_survive_a_restart_and_broken_file_means_auto() {
        let store = crate::vault::test_store("settings");
        assert_eq!(load(&store).language, Language::Auto);
        save(
            &store,
            &Settings {
                language: Language::En,
                theme: Theme::Dark,
                hotwords: vec!["Скайлс".into()],
                models_prompt_seen: true,
            },
        )
        .unwrap();
        assert_eq!(load(&store).language, Language::En);
        assert_eq!(load(&store).hotwords, ["Скайлс"]);
        assert_eq!(load(&store).theme, Theme::Dark);
        assert!(load(&store).models_prompt_seen);
        std::fs::write(store.dir().join(FILE), "{\"language\":\"de\"}").unwrap();
        assert_eq!(load(&store).language, Language::Auto);
        std::fs::write(store.dir().join(FILE), "{}").unwrap();
        assert_eq!(load(&store).language, Language::Auto);
        assert_eq!(load(&store).theme, Theme::Auto);
        assert!(!load(&store).models_prompt_seen);
        std::fs::write(store.dir().join(FILE), "{\"theme\":\"sepia\"}").unwrap();
        assert_eq!(load(&store).theme, Theme::Auto);
    }
}
