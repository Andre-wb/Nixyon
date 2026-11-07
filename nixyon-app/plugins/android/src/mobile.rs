use serde::de::DeserializeOwned;
use tauri::{
    AppHandle, Runtime,
    plugin::{PluginApi, PluginHandle},
};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Speech<R>> {
    let handle = api.register_android_plugin("dev.demo.nixyon.speech", "SpeechPlugin")?;
    Ok(Speech(handle))
}

/// Доступ к Kotlin-части плагина.
pub struct Speech<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Speech<R> {
    /// Загружает модели при первом вызове (в фоне, на стороне Kotlin) и сообщает,
    /// откуда они взяты.
    pub async fn engine_status(&self) -> crate::Result<EngineStatus> {
        self.0
            .run_mobile_plugin_async("engineStatus", ())
            .await
            .map_err(Into::into)
    }

    /// Разбор WAV 16 кГц на устройстве. Ждёт до конца разбора или отмены.
    pub async fn analyze(&self, request: AnalyzeRequest) -> crate::Result<AnalyzeSummary> {
        self.0
            .run_mobile_plugin_async("analyze", request)
            .await
            .map_err(Into::into)
    }

    /// Эмбеддинг голоса по образцу; занимает движок, как разбор записи.
    pub async fn sample_embedding(&self, request: SampleRequest) -> crate::Result<SampleEmbedding> {
        self.0
            .run_mobile_plugin_async("sampleEmbedding", request)
            .await
            .map_err(Into::into)
    }

    /// Ребёнок или взрослый по кускам речи спикеров; занимает движок, как разбор записи.
    pub async fn speaker_ages(&self, request: SpeakerAgesRequest) -> crate::Result<SpeakerAges> {
        self.0
            .run_mobile_plugin_async("speakerAges", request)
            .await
            .map_err(Into::into)
    }

    pub async fn set_system_bars(&self, request: SystemBarsRequest) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("setSystemBars", request)
            .await
            .map_err(Into::into)
    }

    pub async fn cancel_analysis(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("cancelAnalysis", ())
            .await
            .map_err(Into::into)
    }

    pub async fn storage_paths(&self) -> crate::Result<StoragePaths> {
        self.0
            .run_mobile_plugin_async("storagePaths", ())
            .await
            .map_err(Into::into)
    }

    pub async fn start_recording(&self, request: RecordRequest) -> crate::Result<RecordingStarted> {
        self.0
            .run_mobile_plugin_async("startRecording", request)
            .await
            .map_err(Into::into)
    }

    pub async fn stop_recording(&self) -> crate::Result<RecordingSaved> {
        self.0
            .run_mobile_plugin_async("stopRecording", ())
            .await
            .map_err(Into::into)
    }

    /// Запись с микрофона, оборванная падением приложения и восстановленная при этом
    /// запуске; путь пустой, если восстанавливать было нечего. Отдаётся один раз.
    pub async fn recovered_recording(&self) -> crate::Result<RecordingSaved> {
        self.0
            .run_mobile_plugin_async("recoveredRecording", ())
            .await
            .map_err(Into::into)
    }

    pub async fn import_audio(&self, request: ImportRequest) -> crate::Result<ImportOutcome> {
        self.0
            .run_mobile_plugin_async("importAudio", request)
            .await
            .map_err(Into::into)
    }

    pub async fn player_open(&self, request: PlayerOpenRequest) -> crate::Result<PlayerOpened> {
        self.0
            .run_mobile_plugin_async("playerOpen", request)
            .await
            .map_err(Into::into)
    }

    pub async fn player_play(&self, request: PlayRequest) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("playerPlay", request)
            .await
            .map_err(Into::into)
    }

    pub async fn player_pause(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("playerPause", ())
            .await
            .map_err(Into::into)
    }

    pub async fn player_close(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("playerClose", ())
            .await
            .map_err(Into::into)
    }

    pub async fn cancel_import(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("cancelImport", ())
            .await
            .map_err(Into::into)
    }

    pub async fn open_app_settings(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("openAppSettings", ())
            .await
            .map_err(Into::into)
    }

    pub async fn save_file(&self, path: String, name: String, mime: String) -> crate::Result<bool> {
        self.0
            .run_mobile_plugin_async::<SaveOutcome>("saveFile", FileRequest { path, name, mime })
            .await
            .map(|o| o.saved)
            .map_err(Into::into)
    }

    pub async fn share_file(&self, path: String, name: String, mime: String) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("shareFile", FileRequest { path, name, mime })
            .await
            .map_err(Into::into)
    }

    pub async fn storage_key(&self) -> crate::Result<StorageKey> {
        self.0
            .run_mobile_plugin_async("storageKey", ())
            .await
            .map_err(Into::into)
    }

    pub async fn render_pdf(&self, path: String, blocks: String) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("renderPdf", PdfRequest { path, blocks })
            .await
            .map_err(Into::into)
    }

    pub async fn summary_status(&self) -> crate::Result<SummaryStatus> {
        self.0
            .run_mobile_plugin_async("summaryStatus", ())
            .await
            .map_err(Into::into)
    }

    /// Подсказки имён участников из разговора; модель та же, что у протокола, и работает
    /// с ним по очереди. Отмена: `cancel_summary`.
    pub async fn suggest_speaker_names(
        &self,
        request: SpeakerNamesRequest,
    ) -> crate::Result<SpeakerNames> {
        self.0
            .run_mobile_plugin_async("suggestSpeakerNames", request)
            .await
            .map_err(Into::into)
    }

    pub async fn summarize(&self, request: SummaryRequest) -> crate::Result<SummaryText> {
        self.0
            .run_mobile_plugin_async("summarize", request)
            .await
            .map_err(Into::into)
    }

    pub async fn cancel_summary(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("cancelSummary", ())
            .await
            .map_err(Into::into)
    }

    pub async fn models_status(&self) -> crate::Result<ModelsStatus> {
        self.0
            .run_mobile_plugin_async("modelsStatus", ())
            .await
            .map_err(Into::into)
    }

    /// Качает выбранные модели с зеркала. Ждёт до конца загрузки, отмены или ошибки.
    pub async fn download_models(&self, request: DownloadModelsRequest) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("downloadModels", request)
            .await
            .map_err(Into::into)
    }

    pub async fn cancel_model_download(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin_async("cancelModelDownload", ())
            .await
            .map_err(Into::into)
    }
}
