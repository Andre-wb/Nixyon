use serde::Deserialize;

/// Реплика эталона. Секунды от начала записи; текста нет у записей без расшифровки.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TruthTurn {
    pub speaker: u32,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub text: Option<String>,
    /// Кто говорит: имя голоса в наборе (по нему сверяются имена из образцов).
    #[serde(default)]
    pub voice: Option<String>,
}
