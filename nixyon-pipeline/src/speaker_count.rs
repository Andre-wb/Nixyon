/// Сколько людей в записи: число от пользователя или «не знаю».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeakerCount {
    Known(u32),
    Auto,
}
