use serde::{Deserialize, Serialize};

use crate::Word;

/// Пометка хода, где говорящих несколько одновременно.
pub const OVERLAP_FLAG: &str = "overlap";

/// Ход: подряд идущая речь одного спикера.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Turn {
    pub id: u32,
    pub speaker: u32,
    /// Секунды от начала записи.
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// None, если движок не дал время слов.
    pub words: Option<Vec<Word>>,
    /// Пометки вроде "overlap" или "clipping". default: файлы без поля тоже читаются.
    #[serde(default)]
    pub flags: Vec<String>,
}

impl Turn {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    /// Число слов: по времени слов, если оно есть, иначе по тексту.
    pub fn word_count(&self) -> usize {
        match &self.words {
            Some(words) => words.len(),
            None => self.text.split_whitespace().count(),
        }
    }
}
