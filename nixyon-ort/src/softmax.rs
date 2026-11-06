/// Вероятности из логитов. Перед экспонентой вычитается наибольший логит, чтобы большие
/// значения не переполняли f32.
pub fn softmax<const N: usize>(logits: [f32; N]) -> [f32; N] {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = logits.map(|x| (x - max).exp());
    let sum: f32 = exp.iter().sum();
    exp.map(|x| x / sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probabilities_sum_to_one_and_keep_the_order_of_logits() {
        let p = softmax([0.36, -0.61, 0.05]);
        assert!((p.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert!(p[0] > p[2] && p[2] > p[1]);
    }

    #[test]
    fn equal_logits_give_equal_probabilities() {
        assert_eq!(softmax([2.0, 2.0]), [0.5, 0.5]);
    }

    #[test]
    fn huge_logits_do_not_overflow() {
        let p = softmax([1000.0, 0.0, -1000.0]);
        assert!((p[0] - 1.0).abs() < 1e-6);
        assert!(p.iter().all(|x| x.is_finite()));
    }
}
