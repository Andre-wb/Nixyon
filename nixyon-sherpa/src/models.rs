use std::path::Path;

use sherpa_onnx::{
    OfflineSpeakerDiarization, SpeakerEmbeddingExtractor, VadModelConfig, VoiceActivityDetector,
};

use nixyon_core::{BlockSegmenter, LanguageDetector, SpeechEdges, SpeechLanguage, VoiceEmbedder};

use crate::language_recognizers::LanguageRecognizers;
use crate::music_check::MusicCheck;
use crate::parallel::Parallel;
use crate::pyannote_segmenter::PyannoteSegmenter;
use crate::sherpa_voice::SherpaVoice;
use crate::vad_denoiser::VadDenoiser;
use crate::whisper_language::WhisperLanguage;
use crate::{Decoding, ModelManifest, SherpaError, config};

/// Загруженные модели. VAD хранит состояние потока, поэтому здесь только его
/// конфиг, а детектор создаётся на каждую запись.
pub struct SherpaModels {
    version: String,
    vad_config: VadModelConfig,
    tight_vad_config: VadModelConfig,
    /// Распознаватели одного языка: несколько, чтобы куски речи распознавались параллельно.
    recognizers: LanguageRecognizers,
    /// Определение языка записи.
    language: WhisperLanguage,
    /// Модель голоса: по умолчанию из набора, для замера её можно заменить.
    voice: Box<dyn VoiceEmbedder>,
    /// Есть, только если в наборе есть модель нейросегментации или её дали извне.
    segmenter: Option<Box<dyn BlockSegmenter>>,
    music: Option<MusicCheck>,
    /// Шумоподавитель для второго VAD, который режет длинные куски: есть, если в наборе есть
    /// роль denoiser, как в Kotlin-плагине.
    vad_denoiser: Option<VadDenoiser>,
    /// Сколько эмбеддингов кусков и отрезков считается параллельно.
    embed_jobs: usize,
}

impl SherpaModels {
    /// Загрузка как в приложении: поиск по лучам с горячими словами (нужен словарь BPE).
    pub fn load(manifest: &ModelManifest) -> Result<Self, SherpaError> {
        Self::load_with(manifest, Decoding::default())
    }

    pub fn load_with(manifest: &ModelManifest, decoding: Decoding) -> Result<Self, SherpaError> {
        Self::load_with_threads(manifest, decoding, config::threads())
    }

    /// Загрузка с заданным числом потоков на каждую модель. Для прогона многих коротких
    /// записей разом: несколько движков по одному потоку быстрее, чем по четыре.
    pub fn load_with_threads(
        manifest: &ModelManifest,
        decoding: Decoding,
        threads: i32,
    ) -> Result<Self, SherpaError> {
        Self::load_pooled(manifest, decoding, threads, 1)
    }

    /// Загрузка, где эмбеддинги кусков и отрезков считаются в `embed_jobs` потоков, у
    /// каждого свой экстрактор модели голоса. Остальные модели по одной.
    pub fn load_pooled(
        manifest: &ModelManifest,
        decoding: Decoding,
        threads: i32,
        embed_jobs: usize,
    ) -> Result<Self, SherpaError> {
        let parallel = Parallel {
            embed_jobs,
            ..Parallel::default()
        };
        Self::load_parallel(manifest, decoding, threads, parallel)
    }

    /// Загрузка, где эмбеддинги и распознавание кусков речи идут параллельно: на каждый
    /// поток своя копия модели голоса и свой распознаватель.
    pub fn load_parallel(
        manifest: &ModelManifest,
        decoding: Decoding,
        threads: i32,
        parallel: Parallel,
    ) -> Result<Self, SherpaError> {
        let threads = threads.max(1);
        let embed_jobs = parallel.embed_jobs.max(1);
        let asr_jobs = parallel.asr_jobs.max(1);
        let vad_config = config::vad(manifest, SpeechEdges::Padded);
        let tight_vad_config = config::vad(manifest, SpeechEdges::Tight);
        // Пробное создание: ошибка в модели VAD видна при загрузке, а не при первой записи.
        Self::new_vad(&vad_config)?;
        // Сразу грузится русский: основной язык приложения. Английский грузится, когда
        // придёт английская запись.
        let recognizers =
            LanguageRecognizers::load(manifest, SpeechLanguage::Ru, decoding, threads, asr_jobs)?;
        let language = WhisperLanguage::load(manifest)?;
        let embedder = || {
            SpeakerEmbeddingExtractor::create(&config::embedder(manifest, threads))
                .ok_or(SherpaError::CreateFailed("модель голоса"))
        };
        let first = embedder()?;
        let rest = (1..embed_jobs)
            .map(|_| embedder())
            .collect::<Result<Vec<_>, _>>()?;
        let music = MusicCheck::load(manifest)?;
        let vad_denoiser = manifest
            .denoiser_path()
            .map(|path| VadDenoiser::load(Path::new(&path)))
            .transpose()?;
        let segmenter = manifest
            .segmentation_path()
            .map(|path| {
                OfflineSpeakerDiarization::create(&config::diarization(manifest, path, threads))
                    .map(|d| Box::new(PyannoteSegmenter::new(d)) as Box<dyn BlockSegmenter>)
                    .ok_or(SherpaError::CreateFailed("нейросегментация"))
            })
            .transpose()?;
        Ok(Self {
            version: manifest.version().to_string(),
            vad_config,
            tight_vad_config,
            recognizers,
            language,
            voice: Box::new(SherpaVoice::pooled(first, rest)),
            segmenter,
            music,
            vad_denoiser,
            embed_jobs,
        })
    }

    /// Проверка «музыка вместо речи», если в наборе есть распознавание звуков.
    pub(crate) fn music_check(&self) -> Option<&MusicCheck> {
        self.music.as_ref()
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn embedding_dim(&self) -> i32 {
        i32::try_from(self.voice.dim()).unwrap_or(i32::MAX)
    }

    /// Другая модель голоса для всего, что считает эмбеддинги: куски, окна, отрезки
    /// нейросегментации и образцы голоса.
    pub fn with_voice(mut self, voice: Box<dyn VoiceEmbedder>) -> Self {
        self.voice = voice;
        self
    }

    /// Без шумоподавителя из набора: длинные куски VAD не режутся. Для замера, как было до
    /// шумоподавителя.
    pub fn without_vad_denoiser(mut self) -> Self {
        self.vad_denoiser = None;
        self
    }

    pub(crate) fn vad_denoiser(&self) -> Option<&VadDenoiser> {
        self.vad_denoiser.as_ref()
    }

    /// Другая нейросегментация вместо модели из набора.
    pub fn with_segmenter(mut self, segmenter: Box<dyn BlockSegmenter>) -> Self {
        self.segmenter = Some(segmenter);
        self
    }

    pub(crate) fn new_vad(config: &VadModelConfig) -> Result<VoiceActivityDetector, SherpaError> {
        // Буфер VAD на 60 с: сегменты забираются после каждого куска, больше не копится.
        VoiceActivityDetector::create(config, 60.0).ok_or(SherpaError::CreateFailed("VAD"))
    }

    pub(crate) fn vad_config(&self) -> &VadModelConfig {
        &self.vad_config
    }

    pub(crate) fn vad_config_for(&self, edges: SpeechEdges) -> &VadModelConfig {
        match edges {
            SpeechEdges::Padded => &self.vad_config,
            SpeechEdges::Tight => &self.tight_vad_config,
        }
    }

    pub(crate) fn recognizers(&self) -> &LanguageRecognizers {
        &self.recognizers
    }

    /// Сколько кусков речи распознаётся параллельно.
    pub fn asr_jobs(&self) -> usize {
        self.recognizers.jobs()
    }

    pub fn decoding(&self) -> Decoding {
        self.recognizers.decoding()
    }

    /// Определение языка записи.
    pub fn language_detector(&self) -> &dyn LanguageDetector {
        &self.language
    }

    /// Модель голоса, которой считаются все эмбеддинги.
    pub fn voice(&self) -> &dyn VoiceEmbedder {
        self.voice.as_ref()
    }

    /// Нейросегментация, если она есть.
    pub fn segmenter(&self) -> Option<&dyn BlockSegmenter> {
        self.segmenter.as_deref()
    }

    /// Сколько эмбеддингов считается параллельно.
    pub fn embed_jobs(&self) -> usize {
        self.embed_jobs
    }
}
