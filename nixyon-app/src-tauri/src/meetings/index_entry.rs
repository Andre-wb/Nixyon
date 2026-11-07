use serde::{Deserialize, Serialize};

use super::meeting_id::MeetingId;
use super::participant::Participant;

/// Встреча в индексе архива. `file`: имя исходного файла записи, как в расшифровке.
/// `title`: своё название; `None`, и интерфейс называет встречу датой и временем.
/// `participants`: `None` у строк прежней версии, пока их не заполнит подготовка при
/// запуске. `labels`: id меток из общего списка индекса. default у новых полей: индекс
/// прежней версии тоже читается.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: MeetingId,
    pub duration_sec: f64,
    pub file: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub participants: Option<Vec<Participant>>,
    #[serde(default)]
    pub labels: Vec<u32>,
}
