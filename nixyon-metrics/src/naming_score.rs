use std::iter::Sum;
use std::ops::Add;

/// Насколько верно спикерам поставлены имена по образцам голоса. Счёт по людям;
/// по нескольким записям складывается.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NamingScore {
    /// Люди из эталона, у которых есть образец: им нужно имя.
    pub expected: usize,
    /// Спикеры, получившие имя того, кто говорит у них больше всего.
    pub correct: usize,
    /// Спикеры, получившие чужое имя (в том числе образца человека, которого нет).
    pub wrong: usize,
}

impl NamingScore {
    /// Люди с образцом, которых никто не назвал верно.
    pub fn missed(&self) -> usize {
        self.expected.saturating_sub(self.correct)
    }
}

impl Add for NamingScore {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            expected: self.expected + other.expected,
            correct: self.correct + other.correct,
            wrong: self.wrong + other.wrong,
        }
    }
}

impl Sum for NamingScore {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}
