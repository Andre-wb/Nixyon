use nixyon_core::Transcript;
use serde::Serialize;

/// Встреча для экрана расшифровки: расшифровка и своё название из индекса. Название не
/// входит в JSON расшифровки, это формат задания.
#[derive(Debug, PartialEq, Serialize)]
pub struct OpenedMeeting {
    pub transcript: Transcript,
    pub title: Option<String>,
}
