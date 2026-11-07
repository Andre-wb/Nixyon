use serde::{Deserialize, Serialize};

use super::index_entry::IndexEntry;
use super::label::Label;

/// Содержимое `meetings.json`: встречи и общий список меток. default у меток: индекс
/// прежней версии без меток тоже читается.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IndexFile {
    pub meetings: Vec<IndexEntry>,
    #[serde(default)]
    pub labels: Vec<Label>,
}
