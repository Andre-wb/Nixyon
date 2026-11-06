/// Среднее векторов одной длины с весами, например по длине окна. None, если нечего
/// усреднять, длины разные или все веса нулевые.
pub fn weighted_vector_mean(items: &[(Vec<f32>, f64)]) -> Option<Vec<f32>> {
    let dim = items.first()?.0.len();
    let total: f64 = items.iter().map(|(_, w)| w).sum();
    if total <= 0.0 || !total.is_finite() || items.iter().any(|(v, _)| v.len() != dim) {
        return None;
    }
    let mut sum = vec![0.0f64; dim];
    for (v, w) in items {
        for (s, x) in sum.iter_mut().zip(v) {
            *s += f64::from(*x) * w;
        }
    }
    Some(sum.into_iter().map(|s| (s / total) as f32).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_longer_window_weighs_more() {
        let m = weighted_vector_mean(&[(vec![0.0, 4.0], 3.0), (vec![4.0, 0.0], 1.0)]).unwrap();
        assert_eq!(m, [1.0, 3.0]);
    }

    #[test]
    fn nothing_zero_weights_or_mixed_lengths_give_nothing() {
        assert_eq!(weighted_vector_mean(&[]), None);
        assert_eq!(weighted_vector_mean(&[(vec![1.0], 0.0)]), None);
        assert_eq!(
            weighted_vector_mean(&[(vec![1.0], 1.0), (vec![1.0, 2.0], 1.0)]),
            None
        );
    }
}
