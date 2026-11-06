/// Как слово эталона соотносится со словом расшифровки после выравнивания.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordEdit {
    /// Слово распознано верно.
    Same(String),
    /// Вместо слова эталона распознано другое.
    Replaced {
        reference: String,
        hypothesis: String,
    },
    /// Слово эталона пропущено.
    Missed(String),
    /// В расшифровке лишнее слово, которого нет в эталоне.
    Extra(String),
}

impl WordEdit {
    pub fn is_error(&self) -> bool {
        !matches!(self, WordEdit::Same(_))
    }
}
