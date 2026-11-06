#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MetricsError {
    #[error("отрезок спикера {speaker}: начало {start}, конец {end}")]
    BadSpan { speaker: u32, start: f64, end: f64 },
    #[error("отрезок: начало {start}, конец {end}")]
    BadInterval { start: f64, end: f64 },
    #[error("воротник {0} с: нужно конечное неотрицательное число")]
    BadCollar(f64),
}
