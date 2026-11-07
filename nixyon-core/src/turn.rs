use serde::{Deserialize, Serialize};

use crate::{TurnEmotion, Word};

/// Пометка хода, где говорящих несколько одновременно.
pub const OVERLAP_FLAG: &str = "overlap";
/// Пометка хода, где голос слышен, а слова распознались не все: у хода без текста не
/// распознано ничего, у хода с текстом только часть сказанного.
pub const UNCLEAR_FLAG: &str = "unclear";
/// Пометка хода, голос которого не похож уверенно на его спикера: спикер лучший из
/// найденных, но может быть не тем.
pub const SPEAKER_UNSURE_FLAG: &str = "speaker_unsure";

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
    /// Пометки вроде "overlap" или "unclear". default: файлы без поля тоже читаются.
    #[serde(default)]
    pub flags: Vec<String>,
    /// Эмоция по голосу или выбранная вручную; None, если модели нет или она не уверена.
    /// default: файлы без поля тоже читаются.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emotion: Option<TurnEmotion>,
}

impl Turn {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }

    /// Неразборчивая речь: голос был, а текста нет совсем или распознана только часть.
    pub fn is_unclear(&self) -> bool {
        self.has_flag(UNCLEAR_FLAG)
    }

    /// Спикер хода не уверенный: голос мало похож на него.
    pub fn is_speaker_unsure(&self) -> bool {
        self.has_flag(SPEAKER_UNSURE_FLAG)
    }

    /// Число слов: по времени слов, если оно есть, иначе по тексту.
    pub fn word_count(&self) -> usize {
        match &self.words {
            Some(words) => words.len(),
            None => self.text.split_whitespace().count(),
        }
    }
}
