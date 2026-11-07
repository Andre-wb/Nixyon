//! Коды ошибок команд меток.

use crate::transcription::CommandError;

/// Метка с таким названием уже есть.
pub const LABEL_EXISTS: &str = "label_exists";
/// Метки с таким id нет: её удалили, пока интерфейс показывал старый список.
pub const NO_LABEL: &str = "no_label";

/// Индекс не читается или не пишется: подробности в лог, человеку общий текст.
pub fn storage_failed(e: std::io::Error) -> CommandError {
    CommandError::logged(CommandError::STORAGE_FAILED, e)
}
