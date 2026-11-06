use nixyon_metrics::{DerBreakdown, ErrorCount, NamingScore, OverlapScore};

/// Итог по одной записи. Текстовых метрик нет, если в разметке нет текста.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordScore {
    pub name: String,
    pub truth_speakers: u32,
    pub found_speakers: u32,
    pub audio_sec: f64,
    pub processing_sec: f64,
    pub der: DerBreakdown,
    pub wer: Option<ErrorCount>,
    pub cer: Option<ErrorCount>,
    pub cpwer: Option<ErrorCount>,
    pub overlap: OverlapScore,
    /// Имена по образцам голоса; None, если образцов не давали.
    pub naming: Option<NamingScore>,
}

impl RecordScore {
    /// Время обработки к длительности записи; меньше 1: быстрее реального времени.
    pub fn rtf(&self) -> Option<f64> {
        (self.audio_sec > 0.0).then(|| self.processing_sec / self.audio_sec)
    }
}
