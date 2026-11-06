/// Порог слияния групп в одного человека, который прислало устройство вместе с моделью
/// голоса. None: не косинус от -1 до 1, ответ движка битый.
pub fn checked_same_speaker(cosine: f64) -> Option<f64> {
    (cosine.is_finite() && (-1.0..=1.0).contains(&cosine)).then_some(cosine)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_from_minus_one_to_one_is_taken_as_is() {
        for c in [-1.0, 0.0, 0.7, 1.0] {
            assert_eq!(checked_same_speaker(c), Some(c));
        }
    }

    #[test]
    fn value_outside_the_cosine_range_is_refused() {
        for c in [1.01, -1.5, f64::NAN, f64::INFINITY] {
            assert_eq!(checked_same_speaker(c), None);
        }
    }
}
