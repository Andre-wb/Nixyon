use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

/// Что загружено на устройстве: откуда модели и какой набор.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    /// "folder": папка приложения (adb push), "apk": assets внутри APK.
    pub source: String,
    pub version: String,
    pub load_ms: u64,
    pub embedding_dim: u32,
}

/// Папки приложения на устройстве.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoragePaths {
    pub files: String,
    pub cache: String,
    /// Папка на общей памяти, куда кладёт файлы adb push; её может не быть.
    pub external_files: Option<String>,
}

/// Сколько звука уже разобрано и на каком проходе.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeProgress {
    /// "recognize": распознавание, "voices": нейросегментация. Старый плагин поля не шлёт.
    #[serde(default)]
    pub phase: Option<String>,
    pub done_sec: f64,
    pub total_sec: f64,
}

/// Разбор WAV 16 кГц: результаты Kotlin пишет в `out_path`, прогресс шлёт в канал.
/// `window_sec` и `step_sec`: окна для эмбеддингов голоса, 0 и 0 значит без окон.
/// `spans_path` и `neural_block_sec`: нейросегментация, отрезки пишутся в этот файл;
/// None и 0 значит без неё.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRequest {
    pub wav_path: String,
    pub out_path: String,
    pub window_sec: f64,
    pub step_sec: f64,
    pub spans_path: Option<String>,
    pub neural_block_sec: f64,
    /// Горячие слова одной строкой через «/» (`nixyon_core::hotwords_line`); пусто: без них.
    pub hotwords: String,
    pub on_progress: Channel<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeSummary {
    pub segments: u32,
    pub duration_sec: f64,
    /// Имена файлов моделей и версия набора: попадают в расшифровку.
    pub vad_model: String,
    pub asr_models: String,
    pub embedding_model: String,
    /// Порог слияния групп в одного человека, привязанный к модели голоса на устройстве.
    pub same_speaker_cosine: f64,
}

/// Эмбеддинг образца голоса по WAV 16 кГц. Речи меньше `min_speech_sec` секунд:
/// отказ с кодом "sample_too_short".
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleRequest {
    pub wav_path: String,
    pub min_speech_sec: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleEmbedding {
    pub embedding: Vec<f32>,
    /// Имя файла модели голоса, которой посчитан эмбеддинг.
    pub voice_model: String,
}

/// Кусок речи спикера в секундах от начала записи.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgePiece {
    pub start_sec: f64,
    pub end_sec: f64,
}

/// Спикер и куски его речи, по которым модель решает, ребёнок ли это.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgeSpeaker {
    pub id: u32,
    pub pieces: Vec<AgePiece>,
}

/// Запрос «ребёнок или взрослый» по WAV 16 кГц записи.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerAgesRequest {
    pub wav_path: String,
    pub speakers: Vec<AgeSpeaker>,
}

/// Ответ по спикеру. `child`: Some(true) ребёнок, Some(false) взрослый, None модель не
/// уверена или речи не хватило. `female`: Some(true) женский голос, Some(false) мужской.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerAgeVerdict {
    pub id: u32,
    pub child: Option<bool>,
    #[serde(default)]
    pub female: Option<bool>,
}

/// `available`: false, если модели audEERING нет в наборе; тогда `speakers` пуст.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerAges {
    pub available: bool,
    pub speakers: Vec<SpeakerAgeVerdict>,
}

/// Реплика в секундах от начала записи.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EmotionTurn {
    pub id: u32,
    pub start_sec: f64,
    pub end_sec: f64,
}

/// Запрос эмоций реплик по WAV 16 кГц записи.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnEmotionsRequest {
    pub wav_path: String,
    pub turns: Vec<EmotionTurn>,
}

/// Ответ по реплике: `emotion` None, если реплика слишком короткая или модель не уверена.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnEmotionVerdict {
    pub id: u32,
    pub emotion: Option<nixyon_core::TurnEmotion>,
}

/// `available`: false, если модели эмоций нет в наборе или она выключена; тогда `turns`
/// пуст.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnEmotions {
    pub available: bool,
    pub turns: Vec<TurnEmotionVerdict>,
}

/// Тема значков строки состояния и навигации: `dark` для тёмного интерфейса.
#[derive(Serialize)]
pub struct SystemBarsRequest {
    pub dark: bool,
}

/// Начало записи: события (уровень, автоостановка) идут в канал.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordRequest {
    pub on_event: Channel<serde_json::Value>,
    /// Тексты уведомления о записи на языке интерфейса: заголовок и название канала.
    pub notification_title: String,
    pub notification_channel: String,
    /// Образец голоса: пишется в `files/samples` и не стирает запись встречи.
    pub sample: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStarted {
    pub sample_rate: u32,
}

/// Событие записи из Kotlin: уровень громкости, живой текст или остановка без участия
/// пользователя.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecorderEvent {
    Level {
        dbfs: f64,
    },
    /// Последние законченные фразы и текущая фраза живого текста, без спикеров.
    Live {
        lines: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Stopped {
        path: Option<String>,
        duration_sec: f64,
        /// "limit", "storage", "interrupted", "background".
        reason: String,
    },
}

/// Сохранённая запись. path None: ничего не успело записаться.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSaved {
    pub path: Option<String>,
    pub duration_sec: f64,
    pub reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub on_progress: Channel<serde_json::Value>,
}

/// Итог импорта. cancelled: пользователь закрыл выбор файла.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub cancelled: bool,
    pub path: Option<String>,
    pub duration_sec: Option<f64>,
    pub name: Option<String>,
}

/// Файл экспорта для «Сохранить» и «Поделиться».
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRequest {
    pub path: String,
    pub name: String,
    pub mime: String,
}

/// Ключ данных для шифрования файлов: 32 байта. Только для ядра приложения, у плагина
/// нет команд для JS. В лог не пишется.
#[derive(Deserialize)]
pub struct StorageKey {
    pub key: Vec<u8>,
}

/// Вход по отпечатку: включён ли он и есть ли на телефоне отпечаток, чтобы его включить.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiometricStatus {
    pub enabled: bool,
    pub available: bool,
}

#[derive(Serialize)]
pub struct BiometricRequest {
    pub enabled: bool,
}

/// Протокол в PDF: блоки приходят JSON-строкой (как их сериализует `nixyon-render`),
/// Kotlin верстает их в файл `path`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfRequest {
    pub path: String,
    pub blocks: String,
}

/// Есть ли на устройстве языковая модель для протокола встречи.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryStatus {
    pub available: bool,
}

/// Запрос к языковой модели: системная инструкция и текст встречи. `max_tokens`: предел
/// длины ответа; встреча вместе с ним должна уложиться в контекст модели.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
    pub on_progress: Channel<serde_json::Value>,
}

/// Ход работы модели: "reading" (читает встречу, в токенах) или "writing" (пишет ответ,
/// `total`: предел длины ответа).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryProgress {
    pub phase: String,
    pub done: u32,
    pub total: u32,
}

/// Участник расшифровки для подсказки имён. `name`: имя, уже данное пользователем; None,
/// если участник подписан «Спикер N».
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NamingSpeaker {
    pub id: u32,
    pub name: Option<String>,
}

/// Реплика расшифровки для подсказки имён.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NamingTurn {
    pub id: u32,
    pub speaker: u32,
    pub text: String,
}

/// Запрос подсказки имён: участники и реплики последней расшифровки, ход работы модели в канал.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerNamesRequest {
    pub speakers: Vec<NamingSpeaker>,
    pub turns: Vec<NamingTurn>,
    pub on_progress: Channel<serde_json::Value>,
}

/// Подсказка: участника `speaker`, похоже, зовут `name`; `turn`: реплика, из которой это
/// видно. Имя пришло от модели и проверено плагином по тексту разговора, но показывается
/// только как текст.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NameSuggestion {
    pub speaker: u32,
    pub name: String,
    pub turn: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerNames {
    pub suggestions: Vec<NameSuggestion>,
}

/// Ответ модели как есть: недоверенный текст, показывается только как текст.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryText {
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveOutcome {
    /// false: пользователь закрыл диалог, не выбрав место.
    pub saved: bool,
}

/// Открыть исходную запись в плеере; события плеера идут в канал.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerOpenRequest {
    pub path: String,
    pub on_event: Channel<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerOpened {
    pub duration_sec: f64,
}

/// Играть с позиции в секундах; None: с текущего места.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRequest {
    pub from_sec: Option<f64>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerEventKind {
    /// Позиция во время проигрывания, 10 раз в секунду.
    Position,
    Playing,
    /// Пауза: по кнопке, при сворачивании или когда звук забрал звонок.
    Paused,
    Ended,
    Error,
}

/// Событие плеера из Kotlin: вид и позиция в секундах.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PlayerEvent {
    #[serde(rename = "type")]
    pub kind: PlayerEventKind,
    pub sec: f64,
}

/// Модель или набор моделей из каталога в APK. `key`: "speech" (обязательный набор речи),
/// "live", "denoiser", "age", "emotion", "llm". `size`: сколько байт качать.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelPackStatus {
    pub key: String,
    pub required: bool,
    pub size: u64,
    pub installed: bool,
}

/// Что скачано и не идёт ли загрузка, начатая до перезапуска. `pending`: что в ней выбрано,
/// `pending_allow_metered`: разрешена ли ей платная сеть. `metered`: телефон сейчас в
/// платной сети, перед загрузкой интерфейс спрашивает.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelsStatus {
    pub packs: Vec<ModelPackStatus>,
    pub downloading: bool,
    pub pending: Vec<String>,
    pub pending_allow_metered: bool,
    pub metered: bool,
    /// Необязательные модели, выключенные в настройках.
    #[serde(default)]
    pub disabled: Vec<String>,
}

/// Какие необязательные модели выключены; остальные включены.
#[derive(Serialize)]
pub struct ModelsDisabledRequest {
    pub packs: Vec<String>,
}

/// Загрузка моделей с зеркала: ключи `ModelPackStatus`, ход загрузки идёт в канал.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadModelsRequest {
    pub packs: Vec<String>,
    pub allow_metered: bool,
    /// Заголовок системного уведомления о загрузке на языке интерфейса.
    pub notification_title: String,
    pub on_progress: Channel<serde_json::Value>,
}

/// Ход загрузки в байтах. `phase`: "downloading", "waiting" (ждёт сети или Wi-Fi),
/// "checking" (сверка сумм).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelDownloadProgress {
    pub phase: String,
    pub done: u64,
    pub total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_events_from_kotlin_are_understood() {
        let level: RecorderEvent =
            serde_json::from_str(r#"{"type":"level","dbfs":-23.5}"#).unwrap();
        assert_eq!(level, RecorderEvent::Level { dbfs: -23.5 });
        let stopped: RecorderEvent = serde_json::from_str(
            r#"{"type":"stopped","path":"/data/rec-1.wav","durationSec":12.5,"reason":"background"}"#,
        )
        .unwrap();
        assert_eq!(
            stopped,
            RecorderEvent::Stopped {
                path: Some("/data/rec-1.wav".into()),
                duration_sec: 12.5,
                reason: "background".into()
            }
        );
        let empty: RecorderEvent = serde_json::from_str(
            r#"{"type":"stopped","path":null,"durationSec":0,"reason":"interrupted"}"#,
        )
        .unwrap();
        assert!(matches!(empty, RecorderEvent::Stopped { path: None, .. }));
    }

    #[test]
    fn live_text_event_from_kotlin_is_understood() {
        let live: RecorderEvent =
            serde_json::from_str(r#"{"type":"live","lines":["добрый день","как"]}"#).unwrap();
        assert_eq!(
            live,
            RecorderEvent::Live {
                lines: vec!["добрый день".into(), "как".into()]
            }
        );
    }

    #[test]
    fn models_status_and_download_progress_from_kotlin_are_understood() {
        let status: ModelsStatus = serde_json::from_str(
            r#"{"packs":[{"key":"speech","required":true,"size":320000000,"installed":false}],"downloading":true,"pending":["speech","llm"],"pendingAllowMetered":true,"metered":false,"disabled":["llm"]}"#,
        )
        .unwrap();
        assert_eq!(status.packs[0].key, "speech");
        assert!(status.packs[0].required && !status.packs[0].installed);
        assert_eq!(status.pending, ["speech", "llm"]);
        assert!(status.pending_allow_metered);
        assert_eq!(status.disabled, ["llm"]);
        let progress: ModelDownloadProgress =
            serde_json::from_str(r#"{"phase":"waiting","done":10,"total":2293388448}"#).unwrap();
        assert_eq!(progress.total, 2_293_388_448);
    }

    #[test]
    fn player_events_from_kotlin_are_understood() {
        let event: PlayerEvent = serde_json::from_str(r#"{"type":"position","sec":12.3}"#).unwrap();
        assert_eq!(
            event,
            PlayerEvent {
                kind: PlayerEventKind::Position,
                sec: 12.3
            }
        );
        let ended: PlayerEvent = serde_json::from_str(r#"{"type":"ended","sec":214.4}"#).unwrap();
        assert_eq!(ended.kind, PlayerEventKind::Ended);
        assert!(serde_json::from_str::<PlayerEvent>(r#"{"type":"jump","sec":0}"#).is_err());
    }
}
