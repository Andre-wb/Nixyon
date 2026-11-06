use std::iter::Sum;
use std::ops::Add;

/// Насколько найдены места, где говорят одновременно. Секунды; по нескольким записям
/// складываются, доли считаются от суммы.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OverlapScore {
    /// Время перекрытий в эталоне.
    pub reference_sec: f64,
    /// Время, помеченное как перекрытие.
    pub found_sec: f64,
    /// Помеченное время, где в эталоне действительно перекрытие.
    pub hit_sec: f64,
}

impl OverlapScore {
    /// Доля помеченного времени, где перекрытие есть. None: ничего не помечено.
    pub fn precision(&self) -> Option<f64> {
        (self.found_sec > 0.0).then(|| self.hit_sec / self.found_sec)
    }

    /// Доля перекрытий эталона, которые помечены. None: в эталоне перекрытий нет.
    pub fn recall(&self) -> Option<f64> {
        (self.reference_sec > 0.0).then(|| self.hit_sec / self.reference_sec)
    }
}

impl Add for OverlapScore {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            reference_sec: self.reference_sec + other.reference_sec,
            found_sec: self.found_sec + other.found_sec,
            hit_sec: self.hit_sec + other.hit_sec,
        }
    }
}

impl Sum for OverlapScore {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_of_summed_scores_come_from_the_sums() {
        let total: OverlapScore = [
            OverlapScore {
                reference_sec: 2.0,
                found_sec: 1.0,
                hit_sec: 1.0,
            },
            OverlapScore {
                reference_sec: 2.0,
                found_sec: 3.0,
                hit_sec: 1.0,
            },
        ]
        .into_iter()
        .sum();
        assert_eq!(total.precision(), Some(0.5));
        assert_eq!(total.recall(), Some(0.5));
    }

    #[test]
    fn nothing_found_or_nothing_to_find_has_no_rate() {
        let s = OverlapScore::default();
        assert_eq!((s.precision(), s.recall()), (None, None));
    }
}
