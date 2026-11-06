#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PipelineError {
    #[error("обработка отменена")]
    Cancelled,
    #[error("в записи не найдено речи")]
    NoSpeech,
}
