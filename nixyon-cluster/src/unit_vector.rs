/// Вектор единичной длины. None для нулевого вектора и для NaN или бесконечности.
pub fn unit(v: &[f32]) -> Option<Vec<f32>> {
    if v.is_empty() || v.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let norm = v
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    if norm == 0.0 {
        return None;
    }
    Some(v.iter().map(|x| (f64::from(*x) / norm) as f32).collect())
}

/// Косинус между единичными векторами.
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_has_length_one_and_keeps_direction() {
        let u = unit(&[3.0, 4.0]).unwrap();
        assert!((u[0] - 0.6).abs() < 1e-6 && (u[1] - 0.8).abs() < 1e-6);
        assert!((cosine(&u, &u) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn zero_empty_and_non_finite_vectors_have_no_direction() {
        assert_eq!(unit(&[0.0, 0.0]), None);
        assert_eq!(unit(&[]), None);
        assert_eq!(unit(&[1.0, f32::NAN]), None);
        assert_eq!(unit(&[f32::INFINITY, 1.0]), None);
    }

    #[test]
    fn opposite_and_orthogonal_vectors() {
        let a = unit(&[1.0, 0.0]).unwrap();
        assert!((cosine(&a, &unit(&[-2.0, 0.0]).unwrap()) + 1.0).abs() < 1e-9);
        assert!(cosine(&a, &unit(&[0.0, 5.0]).unwrap()).abs() < 1e-9);
    }
}
