use serde::{Deserialize, Serialize};

/// Кто писал тексты набора: от этого зависит, насколько WER на нём говорит о модели.
/// Машинный текст сам содержит ошибки, и они попадают в WER.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceSource {
    /// Текст записал или проверил человек.
    Human,
    /// Текст получен автоматически, например подгонкой готового текста к звуку.
    Auto,
    /// Неизвестно; так же читается описание набора без этого поля.
    #[default]
    Unknown,
}

impl ReferenceSource {
    pub fn label(self) -> &'static str {
        match self {
            ReferenceSource::Human => "человек",
            ReferenceSource::Auto => "машина",
            ReferenceSource::Unknown => "неизвестно",
        }
    }
}
