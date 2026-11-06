use std::ops::Add;

use nixyon_metrics::DerBreakdown;
use serde::Serialize;

/// Составляющие DER клипа или набора в секундах: сколько речи в эталоне, сколько
/// пропущено, сколько лишней речи и сколько приписано не тому спикеру.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct ClipDer {
    pub reference_speech: f64,
    pub missed: f64,
    pub false_alarm: f64,
    pub confusion: f64,
}

impl ClipDer {
    /// (пропуск + ложная речь + путаница) / речь эталона; None, если речи нет.
    pub fn rate(&self) -> Option<f64> {
        (self.reference_speech > 0.0)
            .then(|| (self.missed + self.false_alarm + self.confusion) / self.reference_speech)
    }

    /// Доля одной составляющей от речи эталона.
    pub fn share(&self, part: f64) -> Option<f64> {
        (self.reference_speech > 0.0).then(|| part / self.reference_speech)
    }
}

impl From<DerBreakdown> for ClipDer {
    fn from(d: DerBreakdown) -> Self {
        Self {
            reference_speech: d.reference_speech,
            missed: d.missed,
            false_alarm: d.false_alarm,
            confusion: d.confusion,
        }
    }
}

impl Add for ClipDer {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            reference_speech: self.reference_speech + other.reference_speech,
            missed: self.missed + other.missed,
            false_alarm: self.false_alarm + other.false_alarm,
            confusion: self.confusion + other.confusion,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_of_a_set_is_summed_seconds_over_summed_speech() {
        let a = ClipDer {
            reference_speech: 10.0,
            missed: 1.0,
            false_alarm: 0.0,
            confusion: 1.0,
        };
        let b = ClipDer {
            reference_speech: 30.0,
            missed: 0.0,
            false_alarm: 2.0,
            confusion: 0.0,
        };
        let total = a + b;
        assert_eq!(total.rate(), Some(0.1));
        assert_eq!(total.share(total.false_alarm), Some(0.05));
        assert_eq!(ClipDer::default().rate(), None);
    }
}
