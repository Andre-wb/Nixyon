/// Спикер на окне внутри куска речи после пересегментации. Секунды от начала записи.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowLabel {
    pub start: f64,
    pub end: f64,
    pub speaker: u32,
}
