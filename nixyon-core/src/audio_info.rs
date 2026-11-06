use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AudioInfo {
    pub file: String,
    pub duration_sec: f64,
    pub sample_rate: u32,
}
