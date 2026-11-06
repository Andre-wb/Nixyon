use serde::Serialize;

/// Ошибка для интерфейса: только код, текст по коду берётся из словаря. Подробности
/// пишутся в лог там, где ошибка возникла.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommandError {
    pub code: String,
}

impl CommandError {
    pub const BUSY: &'static str = "busy";
    pub const BAD_PATH: &'static str = "bad_path";
    pub const BAD_SPEAKERS: &'static str = "bad_speakers";
    pub const BAD_AUDIO: &'static str = "bad_audio";
    pub const TOO_SHORT: &'static str = "too_short";
    pub const SILENT: &'static str = "silent";
    pub const NO_SPEECH: &'static str = "no_speech";
    pub const CANCELLED: &'static str = "cancelled";
    pub const ENGINE_FAILED: &'static str = "engine_failed";
    pub const STORAGE_FAILED: &'static str = "storage_failed";
    pub const BAD_VOICE_NAME: &'static str = "bad_voice_name";
    pub const VOICE_NAME_TAKEN: &'static str = "voice_name_taken";
    pub const SAMPLE_TOO_SHORT: &'static str = "sample_too_short";
    pub const UNKNOWN_VOICE: &'static str = "unknown_voice";
    pub const HOTWORD_TOO_LONG: &'static str = "hotword_too_long";
    pub const TOO_MANY_HOTWORDS: &'static str = "too_many_hotwords";

    pub fn new(code: &str) -> Self {
        Self { code: code.into() }
    }

    pub fn logged(code: &str, detail: impl std::fmt::Display) -> Self {
        log::error!("{code}: {detail}");
        Self::new(code)
    }
}
