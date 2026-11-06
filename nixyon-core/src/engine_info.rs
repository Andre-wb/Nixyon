use serde::{Deserialize, Serialize};

/// Какие модели дали расшифровку.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EngineInfo {
    pub vad: String,
    pub asr: String,
    pub embedding: String,
}
