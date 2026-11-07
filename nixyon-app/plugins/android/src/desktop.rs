use serde::de::DeserializeOwned;
use tauri::{AppHandle, Runtime, plugin::PluginApi};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Speech<R>> {
    Ok(Speech(app.clone()))
}

/// На ноутбуке движка в приложении нет: распознавание работает только на Android.
pub struct Speech<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Speech<R> {
    pub async fn engine_status(&self) -> crate::Result<EngineStatus> {
        Err(crate::Error::Unsupported)
    }

    pub async fn analyze(&self, _request: AnalyzeRequest) -> crate::Result<AnalyzeSummary> {
        Err(crate::Error::Unsupported)
    }

    pub async fn sample_embedding(
        &self,
        _request: SampleRequest,
    ) -> crate::Result<SampleEmbedding> {
        Err(crate::Error::Unsupported)
    }

    pub async fn speaker_ages(&self, _request: SpeakerAgesRequest) -> crate::Result<SpeakerAges> {
        Err(crate::Error::Unsupported)
    }

    pub async fn turn_emotions(
        &self,
        _request: TurnEmotionsRequest,
    ) -> crate::Result<TurnEmotions> {
        Err(crate::Error::Unsupported)
    }

    pub async fn set_system_bars(&self, _request: SystemBarsRequest) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn cancel_analysis(&self) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn storage_paths(&self) -> crate::Result<StoragePaths> {
        Err(crate::Error::Unsupported)
    }

    pub async fn start_recording(
        &self,
        _request: RecordRequest,
    ) -> crate::Result<RecordingStarted> {
        Err(crate::Error::Unsupported)
    }

    pub async fn stop_recording(&self) -> crate::Result<RecordingSaved> {
        Err(crate::Error::Unsupported)
    }

    /// Записи с микрофона на ноутбуке нет, восстанавливать нечего.
    pub async fn recovered_recording(&self) -> crate::Result<RecordingSaved> {
        Ok(RecordingSaved {
            path: None,
            duration_sec: 0.0,
            reason: "crashed".into(),
        })
    }

    pub async fn import_audio(&self, _request: ImportRequest) -> crate::Result<ImportOutcome> {
        Err(crate::Error::Unsupported)
    }

    pub async fn player_open(&self, _request: PlayerOpenRequest) -> crate::Result<PlayerOpened> {
        Err(crate::Error::Unsupported)
    }

    pub async fn player_play(&self, _request: PlayRequest) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn player_pause(&self) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn player_close(&self) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn cancel_import(&self) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn open_app_settings(&self) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn save_file(
        &self,
        _path: String,
        _name: String,
        _mime: String,
    ) -> crate::Result<bool> {
        Err(crate::Error::Unsupported)
    }

    pub async fn share_file(
        &self,
        _path: String,
        _name: String,
        _mime: String,
    ) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn storage_key(&self) -> crate::Result<StorageKey> {
        Err(crate::Error::Unsupported)
    }

    pub async fn reset_storage_key(&self) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn render_pdf(&self, _path: String, _blocks: String) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn summary_status(&self) -> crate::Result<SummaryStatus> {
        Err(crate::Error::Unsupported)
    }

    pub async fn suggest_speaker_names(
        &self,
        _request: SpeakerNamesRequest,
    ) -> crate::Result<SpeakerNames> {
        Err(crate::Error::Unsupported)
    }

    pub async fn summarize(&self, _request: SummaryRequest) -> crate::Result<SummaryText> {
        Err(crate::Error::Unsupported)
    }

    pub async fn cancel_summary(&self) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn models_status(&self) -> crate::Result<ModelsStatus> {
        Err(crate::Error::Unsupported)
    }

    pub async fn download_models(&self, _request: DownloadModelsRequest) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn set_models_disabled(&self, _request: ModelsDisabledRequest) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub async fn cancel_model_download(&self) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }
}
