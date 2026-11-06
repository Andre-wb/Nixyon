/// Отрезок времени, когда говорит спикер. Секунды от начала записи.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakerSpan {
    pub speaker: u32,
    pub start: f64,
    pub end: f64,
}
