use serde::{Deserialize, Serialize};

/// Сколько людей говорит в клипах набора: от этого зависит, что проверять у спикеров.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetKind {
    /// В каждом клипе один голос: лишний найденный спикер считается ошибкой.
    Single,
    /// Разговор нескольких людей: найденное число голосов сверяется с настоящим.
    Dialog,
    /// Вперемешку: число голосов не проверяется.
    Mixed,
}

impl DatasetKind {
    pub fn label(self) -> &'static str {
        match self {
            DatasetKind::Single => "один голос",
            DatasetKind::Dialog => "разговор",
            DatasetKind::Mixed => "вперемешку",
        }
    }
}
