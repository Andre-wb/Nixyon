use nixyon_core::SpeechEdges;
use sherpa_onnx::{
    OfflineRecognizerConfig, OfflineSpeakerDiarizationConfig, OfflineTransducerModelConfig,
    SileroVadModelConfig, SpeakerEmbeddingExtractorConfig, VadModelConfig,
};

use crate::{Decoding, ModelManifest, ModelRole, SherpaError};

pub const SAMPLE_RATE: i32 = 16_000;

// Значения совпадают с SpeechModels.kt в Kotlin-плагине. В Rust-крейте Default
// у конфигов VAD выведен через derive и даёт нули, поэтому всё задаётся явно.
// Порог низкий: при 0,5 VAD пропускал тихую речь и короткие слова целиком, а ложный
// кусок без речи модель распознаёт пустым текстом, и он отбрасывается.
const VAD_THRESHOLD: f32 = 0.2;
// Строгий порог для деления на спикеров по кускам VAD, без нейросегментации.
const TIGHT_VAD_THRESHOLD: f32 = 0.5;
const VAD_MIN_SILENCE_SECONDS: f32 = 0.25;
const VAD_MIN_SPEECH_SECONDS: f32 = 0.25;
const VAD_WINDOW_SIZE: i32 = 512;
const VAD_MAX_SPEECH_SECONDS: f32 = 20.0;
/// Запас звука с каждой стороны куска речи перед распознаванием, в сэмплах (0,2 с).
pub const SPEECH_PAD_SAMPLES: usize = 3_200;
/// Запись не длиннее этого (20 с, столько же, сколько самый длинный кусок VAD), в
/// которой VAD не нашёл речи, распознаётся целиком: VAD пропускает короткое тихое
/// слово, а модель его слышит.
pub const WHOLE_IF_NO_SPEECH_SAMPLES: usize = 320_000;
/// Группа кусков для распознавания не длиннее 20 с, как самый длинный кусок VAD.
pub const MAX_GROUP_SAMPLES: usize = 320_000;
/// Пауза внутри группы до 2 с: звук паузы модель тоже слушает, а паузы длиннее почти
/// всегда стоят между фразами. Значения те же, что у приложения на Android.
pub const MAX_GAP_SAMPLES: usize = 32_000;
/// Кусок VAD от 15 с со шумоподавителем режется по звуку после него: в шуме VAD склеивает
/// в такие куски речь нескольких людей. От 10 с портились запись с наложением голосов и
/// шум 5 дБ, от 20 с терялся выигрыш на шуме 20 дБ. Как LONG_PIECE_SECONDS в SpeechFinder.kt.
pub const LONG_PIECE_SAMPLES: usize = 240_000;
const GIGAAM_MODEL_TYPE: &str = "nemo_transducer";
// Горячие слова разбиваются на токены словарём BPE той же модели.
const HOTWORDS_MODELING_UNIT: &str = "bpe";
const MAX_THREADS: usize = 4;

pub fn threads() -> i32 {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    // MAX_THREADS маленький, поэтому приведение к i32 без потерь.
    cores.clamp(1, MAX_THREADS) as i32
}

pub fn vad(manifest: &ModelManifest, edges: SpeechEdges) -> VadModelConfig {
    VadModelConfig {
        silero_vad: SileroVadModelConfig {
            model: Some(manifest.path(ModelRole::Vad)),
            threshold: match edges {
                SpeechEdges::Padded => VAD_THRESHOLD,
                SpeechEdges::Tight => TIGHT_VAD_THRESHOLD,
            },
            min_silence_duration: VAD_MIN_SILENCE_SECONDS,
            min_speech_duration: VAD_MIN_SPEECH_SECONDS,
            window_size: VAD_WINDOW_SIZE,
            max_speech_duration: VAD_MAX_SPEECH_SECONDS,
        },
        sample_rate: SAMPLE_RATE,
        num_threads: 1,
        provider: Some("cpu".into()),
        ..VadModelConfig::default()
    }
}

pub fn recognizer(
    manifest: &ModelManifest,
    decoding: Decoding,
    threads: i32,
) -> Result<OfflineRecognizerConfig, SherpaError> {
    let mut config = OfflineRecognizerConfig::default();
    // У GigaAM признаки (64 мел-полосы и т. п.) задаёт сама модель через метаданные,
    // значения feat_config по умолчанию будут переопределены.
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(manifest.path(ModelRole::AsrEncoder)),
        decoder: Some(manifest.path(ModelRole::AsrDecoder)),
        joiner: Some(manifest.path(ModelRole::AsrJoiner)),
    };
    config.model_config.tokens = Some(manifest.path(ModelRole::AsrTokens));
    config.model_config.model_type = Some(GIGAAM_MODEL_TYPE.into());
    config.model_config.num_threads = threads;
    config.model_config.provider = Some("cpu".into());
    match decoding {
        Decoding::Greedy => config.decoding_method = Some("greedy_search".into()),
        Decoding::HotwordBeam { score, paths } => {
            config.decoding_method = Some("modified_beam_search".into());
            config.model_config.modeling_unit = Some(HOTWORDS_MODELING_UNIT.into());
            config.model_config.bpe_vocab =
                Some(manifest.bpe_vocab_path().ok_or(SherpaError::NoBpeVocab)?);
            config.hotwords_score = score;
            config.max_active_paths = paths;
        }
    }
    Ok(config)
}

pub fn embedder(manifest: &ModelManifest, threads: i32) -> SpeakerEmbeddingExtractorConfig {
    SpeakerEmbeddingExtractorConfig {
        model: Some(manifest.path(ModelRole::Embedding)),
        num_threads: threads,
        ..SpeakerEmbeddingExtractorConfig::default()
    }
}

/// Встроенная диаризация sherpa-onnx с моделью pyannote. Её номера говорящих конвейер
/// не берёт, нужны только отрезки, поэтому кластеризация и длительности по умолчанию.
pub fn diarization(
    manifest: &ModelManifest,
    segmentation: String,
    threads: i32,
) -> OfflineSpeakerDiarizationConfig {
    let mut config = OfflineSpeakerDiarizationConfig::default();
    config.segmentation.pyannote.model = Some(segmentation);
    config.segmentation.num_threads = threads;
    config.embedding = embedder(manifest, threads);
    config
}
