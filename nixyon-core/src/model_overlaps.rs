use serde::{Deserialize, Serialize};

use crate::PowersetWindow;

/// Перекрытия, найденные моделью сегментации по всей записи, и окна модели, которые их
/// видят: по окнам второй говорящий перекрытия сопоставляется говорящим расшифровки.
/// Остальные окна не хранятся, чтобы на длинной записи не держать их в памяти.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelOverlaps {
    /// Начало и конец каждого перекрытия, секунды. По времени.
    pub spans: Vec<(f64, f64)>,
    /// Окна, которые заходят хотя бы в одно перекрытие. По времени.
    pub windows: Vec<PowersetWindow>,
}
