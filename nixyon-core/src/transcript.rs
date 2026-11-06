use serde::{Deserialize, Serialize};

use crate::{AudioInfo, EngineInfo, Speaker, Turn};

/// Версия формата файла расшифровки.
pub const SCHEMA_VERSION: u32 = 1;

/// Расшифровка целиком. Все форматы экспорта строятся из неё.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Transcript {
    pub schema_version: u32,
    /// RFC 3339, UTC. Время задаёт платформа: ядро часы не читает.
    pub created_at: String,
    pub audio: AudioInfo,
    pub engine: EngineInfo,
    pub speakers: Vec<Speaker>,
    pub turns: Vec<Turn>,
}

impl Transcript {
    pub fn speaker(&self, id: u32) -> Option<&Speaker> {
        self.speakers.iter().find(|s| s.id == id)
    }

    pub fn speaker_mut(&mut self, id: u32) -> Option<&mut Speaker> {
        self.speakers.iter_mut().find(|s| s.id == id)
    }
}
