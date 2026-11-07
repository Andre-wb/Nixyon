use serde::{Deserialize, Serialize};

use super::meeting_id::MeetingId;

/// Встреча в индексе архива. `file`: имя исходного файла записи, как в расшифровке.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: MeetingId,
    pub duration_sec: f64,
    pub file: String,
}
