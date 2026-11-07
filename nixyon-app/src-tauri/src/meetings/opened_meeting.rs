use nixyon_core::Transcript;
use serde::Serialize;

/// Встреча для экрана расшифровки: расшифровка, своё название из индекса и есть ли ещё
/// исходная запись. Название и звук не входят в JSON расшифровки, это формат задания.
#[derive(Debug, PartialEq, Serialize)]
pub struct OpenedMeeting {
    pub transcript: Transcript,
    pub title: Option<String>,
    /// false: звук удалён очисткой, слушать и пересчитывать реплики нечем.
    pub has_audio: bool,
}
