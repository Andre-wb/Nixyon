#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    #[error("нет спикера {0}")]
    UnknownSpeaker(u32),
    #[error("нельзя слить спикера с самим собой")]
    SameSpeaker,
    #[error("имя длиннее {max} символов")]
    NameTooLong { max: usize },
    #[error("в имени есть управляющие символы")]
    NameHasControlChars,
    #[error("нет хода {0}")]
    UnknownTurn(u32),
    #[error("пустой текст реплики")]
    TextEmpty,
    #[error("текст реплики длиннее {max} символов")]
    TextTooLong { max: usize },
    #[error("в тексте реплики есть управляющие символы")]
    TextHasControlChars,
}
