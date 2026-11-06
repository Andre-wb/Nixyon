/// Среднее нескольких эмбеддингов, приведённое к длине 1: эмбеддинг длинного куска по
/// его окнам. Пусто, если эмбеддингов нет или их сумма нулевая.
pub fn mean_unit(embeddings: &[Vec<f32>]) -> Vec<f32> {
    let Some(first) = embeddings.first() else {
        return Vec::new();
    };
    let mut sum = vec![0.0f32; first.len()];
    for e in embeddings {
        for (s, v) in sum.iter_mut().zip(e) {
            *s += v;
        }
    }
    let norm = sum.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm <= f32::EPSILON || !norm.is_finite() {
        return Vec::new();
    }
    sum.iter().map(|v| v / norm).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_of_two_directions_is_their_unit_bisector() {
        let m = mean_unit(&[vec![1.0, 0.0], vec![0.0, 1.0]]);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!((m[0] - h).abs() < 1e-6 && (m[1] - h).abs() < 1e-6);
    }

    #[test]
    fn single_embedding_is_scaled_to_unit_length() {
        assert_eq!(mean_unit(&[vec![3.0, 4.0]]), [0.6, 0.8]);
    }

    #[test]
    fn nothing_or_opposite_vectors_give_nothing() {
        assert!(mean_unit(&[]).is_empty());
        assert!(mean_unit(&[vec![1.0, 0.0], vec![-1.0, 0.0]]).is_empty());
    }
}
