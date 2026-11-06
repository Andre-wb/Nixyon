#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),
    #[error("это не WAV: {0}")]
    NotWav(&'static str),
    #[error("формат WAV не поддерживается: код {format_tag}, {bits} бит")]
    UnsupportedFormat { format_tag: u16, bits: u16 },
    #[error("недопустимые параметры WAV: {0}")]
    BadFormat(&'static str),
    #[error("запись длиннее, чем помещается в WAV")]
    TooLong,
    #[error("ошибка ресемплинга: {0}")]
    Resample(String),
}
