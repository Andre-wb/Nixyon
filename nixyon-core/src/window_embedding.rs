/// Эмбеддинг голоса на окне внутри куска речи.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowEmbedding {
    /// Секунды от начала записи.
    pub start: f64,
    pub end: f64,
    pub embedding: Vec<f32>,
}
