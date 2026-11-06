use std::iter::Sum;
use std::ops::Add;

/// Число ошибок (замены, вставки, удаления) и длина эталона в словах или знаках.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ErrorCount {
    pub errors: usize,
    pub reference_len: usize,
}

impl ErrorCount {
    /// Доля ошибок; None, если эталон пустой.
    pub fn rate(&self) -> Option<f64> {
        // Счётчики слов далеко от 2^53, перевод в f64 точный.
        (self.reference_len > 0).then(|| self.errors as f64 / self.reference_len as f64)
    }
}

impl Add for ErrorCount {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            errors: self.errors + other.errors,
            reference_len: self.reference_len + other.reference_len,
        }
    }
}

impl Sum for ErrorCount {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_of_summed_counts_is_errors_over_total_reference() {
        let total: ErrorCount = [
            ErrorCount {
                errors: 1,
                reference_len: 4,
            },
            ErrorCount {
                errors: 3,
                reference_len: 4,
            },
        ]
        .into_iter()
        .sum();
        assert_eq!(total.rate(), Some(0.5));
    }

    #[test]
    fn empty_reference_has_no_rate() {
        assert_eq!(ErrorCount::default().rate(), None);
    }
}
