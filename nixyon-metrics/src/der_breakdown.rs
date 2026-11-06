use std::iter::Sum;
use std::ops::Add;

/// Составляющие DER в секундах. Перекрытая речь считается за каждого спикера.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DerBreakdown {
    /// Речь эталона в оцениваемых участках.
    pub reference_speech: f64,
    /// Эталон говорит, а гипотеза молчит или говорит меньше людей.
    pub missed: f64,
    /// Гипотеза говорит там, где в эталоне никого или меньше людей.
    pub false_alarm: f64,
    /// Речь приписана не тому спикеру.
    pub confusion: f64,
}

impl DerBreakdown {
    /// (пропуск + ложная речь + путаница) / речь эталона; None, если речи нет.
    pub fn rate(&self) -> Option<f64> {
        (self.reference_speech > 0.0)
            .then(|| (self.missed + self.false_alarm + self.confusion) / self.reference_speech)
    }
}

impl Add for DerBreakdown {
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

impl Sum for DerBreakdown {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_sums_all_three_kinds_of_error() {
        let d = DerBreakdown {
            reference_speech: 10.0,
            missed: 1.0,
            false_alarm: 0.5,
            confusion: 1.5,
        };
        assert_eq!(d.rate(), Some(0.3));
    }

    #[test]
    fn no_reference_speech_has_no_rate() {
        assert_eq!(DerBreakdown::default().rate(), None);
    }

    #[test]
    fn records_add_up_before_the_rate_is_taken() {
        let a = DerBreakdown {
            reference_speech: 2.0,
            missed: 1.0,
            ..DerBreakdown::default()
        };
        let b = DerBreakdown {
            reference_speech: 8.0,
            ..DerBreakdown::default()
        };
        assert_eq!([a, b].into_iter().sum::<DerBreakdown>().rate(), Some(0.1));
    }
}
