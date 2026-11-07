//! Видеокарта (Vulkan) для языковой модели в настройках. По умолчанию выключена; есть только
//! в APK, собранном с ней. Решает и запоминает Kotlin-плагин, здесь команды для интерфейса.

use nixyon_android::{SpeechExt, SummaryStatus};
use serde::Serialize;
use tauri::{AppHandle, Runtime};

use super::llm_error;
use crate::transcription::CommandError;

/// `built`: APK собран с видеокартой, `enabled`: включена в настройках, `device`: на чём
/// модель считала в последний раз («CPU» или «Vulkan: …»), `None`, если ещё не грузилась.
#[derive(Debug, Serialize, PartialEq)]
pub struct LlmGpuStatus {
    pub built: bool,
    pub enabled: bool,
    pub device: Option<String>,
}

impl From<SummaryStatus> for LlmGpuStatus {
    fn from(status: SummaryStatus) -> Self {
        Self {
            built: status.gpu_built,
            enabled: status.gpu_enabled,
            device: status.gpu_device,
        }
    }
}

/// `None` на ноутбуке: интерфейс тогда прячет настройку.
#[tauri::command]
pub async fn llm_gpu_status<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<LlmGpuStatus>, CommandError> {
    match app.speech().summary_status().await {
        Ok(status) => Ok(Some(status.into())),
        Err(nixyon_android::Error::Unsupported) => Ok(None),
        Err(e) => Err(llm_error(e)),
    }
}

/// Действует со следующей загрузки модели. В APK без видеокарты включение отвечает
/// `llm_gpu_not_built`.
#[tauri::command]
pub async fn set_llm_gpu<R: Runtime>(app: AppHandle<R>, enabled: bool) -> Result<(), CommandError> {
    app.speech().set_llm_gpu(enabled).await.map_err(llm_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_for_settings_comes_from_the_plugin_answer() {
        let status: SummaryStatus = serde_json::from_str(
            r#"{"available":true,"gpuBuilt":true,"gpuEnabled":false,"gpuDevice":"CPU"}"#,
        )
        .unwrap();
        let gpu = LlmGpuStatus::from(status);
        assert_eq!(
            gpu,
            LlmGpuStatus {
                built: true,
                enabled: false,
                device: Some("CPU".into())
            }
        );
        assert_eq!(
            serde_json::to_string(&gpu).unwrap(),
            r#"{"built":true,"enabled":false,"device":"CPU"}"#
        );
    }
}
