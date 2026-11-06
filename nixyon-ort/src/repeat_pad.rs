/// Кусок, повторённый подряд до длины `len`: модель с входом постоянной длины получает
/// только этот голос, без тишины. Пустой кусок даёт пустой результат.
pub fn repeat_pad(samples: &[f32], len: usize) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    samples.iter().copied().cycle().take(len).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_piece_repeats_until_the_length_is_filled() {
        assert_eq!(
            repeat_pad(&[1.0, 2.0, 3.0], 7),
            [1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0]
        );
    }

    #[test]
    fn piece_of_exact_length_stays_the_same() {
        assert_eq!(repeat_pad(&[1.0, 2.0], 2), [1.0, 2.0]);
    }

    #[test]
    fn empty_piece_gives_nothing() {
        assert!(repeat_pad(&[], 5).is_empty());
    }
}
