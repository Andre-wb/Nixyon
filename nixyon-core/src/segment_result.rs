use crate::WindowEmbedding;

/// Результат движка по одному куску речи.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentResult {
    /// Секунды от начала записи.
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub tokens: Vec<String>,
    /// Время каждого токена в секундах от начала куска.
    pub token_times: Vec<f32>,
    /// Пустой, если кусок слишком короткий для модели голоса.
    pub embedding: Vec<f32>,
    /// Эмбеддинги по окнам внутри куска; пусто, если окна не запрошены
    /// или кусок короче окна.
    pub windows: Vec<WindowEmbedding>,
}
