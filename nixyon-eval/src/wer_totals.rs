use serde::Serialize;

use crate::{DatasetScore, ErrorTotal, ReferenceSource};

/// WER, сложенный по нескольким наборам, и сколько наборов в него вошло.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SetsTotal {
    pub sets: usize,
    pub wer: ErrorTotal,
}

/// Общий WER прогона: по наборам, где тексты писал человек, и по всем наборам с
/// текстом. None: таких наборов в прогоне нет.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WerTotals {
    pub human: Option<SetsTotal>,
    pub all: Option<SetsTotal>,
}

pub fn wer_totals(scores: &[DatasetScore]) -> WerTotals {
    let total = |keep: &dyn Fn(&DatasetScore) -> bool| {
        let chosen: Vec<&DatasetScore> = scores
            .iter()
            .filter(|s| s.info.has_text && keep(s))
            .collect();
        (!chosen.is_empty()).then(|| SetsTotal {
            sets: chosen.len(),
            wer: ErrorTotal::of(chosen.iter().map(|s| {
                (
                    s.summary.overall.wer.errors,
                    s.summary.overall.wer.reference,
                )
            })),
        })
    };
    WerTotals {
        human: total(&|s| s.info.reference == ReferenceSource::Human),
        all: total(&|_| true),
    }
}
