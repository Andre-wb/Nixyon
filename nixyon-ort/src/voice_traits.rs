use crate::softmax::softmax;

/// Выход модели возраста: доля от 0 до 1 на 100 лет.
const AGE_SCALE_YEARS: f32 = 100.0;

/// Оценка голоса моделью audEERING: возраст и вероятности трёх классов, в сумме 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceTraits {
    pub age_years: f32,
    pub female: f32,
    pub male: f32,
    pub child: f32,
}

impl VoiceTraits {
    /// Из выходов модели: `logits_age` и `logits_gender` в порядке female, male, child.
    pub fn from_logits(age: f32, gender: [f32; 3]) -> Self {
        let [female, male, child] = softmax(gender);
        Self {
            age_years: age * AGE_SCALE_YEARS,
            female,
            male,
            child,
        }
    }

    /// Среднее с весами, например по длине окна: оценка длинного куска по его окнам или
    /// спикера по его кускам. None, если нечего усреднять или все веса нулевые.
    pub fn weighted_mean(items: &[(Self, f64)]) -> Option<Self> {
        let total: f64 = items.iter().map(|(_, w)| w).sum();
        if items.is_empty() || total <= 0.0 || !total.is_finite() {
            return None;
        }
        let mean = |field: fn(&Self) -> f32| {
            (items
                .iter()
                .map(|(t, w)| f64::from(field(t)) * w)
                .sum::<f64>()
                / total) as f32
        };
        Some(Self {
            age_years: mean(|t| t.age_years),
            female: mean(|t| t.female),
            male: mean(|t| t.male),
            child: mean(|t| t.child),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logits_become_probabilities_in_the_model_order() {
        let t = VoiceTraits::from_logits(0.3, [3.0, 0.0, 0.0]);
        assert!((t.age_years - 30.0).abs() < 1e-4);
        assert!(t.female > t.male);
        assert!((t.male - t.child).abs() < 1e-6);
        assert!((t.female + t.male + t.child - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_mean_weighs_each_item_by_its_weight() {
        let a = VoiceTraits {
            age_years: 10.0,
            female: 1.0,
            male: 0.0,
            child: 0.0,
        };
        let b = VoiceTraits {
            age_years: 40.0,
            female: 0.0,
            male: 1.0,
            child: 0.0,
        };
        let m = VoiceTraits::weighted_mean(&[(a, 1.0), (b, 3.0)]).unwrap();
        assert!((m.age_years - 32.5).abs() < 1e-4);
        assert!((m.female - 0.25).abs() < 1e-6);
        assert!((m.male - 0.75).abs() < 1e-6);
    }

    #[test]
    fn nothing_to_average_gives_nothing() {
        assert_eq!(VoiceTraits::weighted_mean(&[]), None);
        let t = VoiceTraits::from_logits(0.0, [0.0; 3]);
        assert_eq!(VoiceTraits::weighted_mean(&[(t, 0.0)]), None);
    }
}
