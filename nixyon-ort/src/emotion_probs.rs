use nixyon_core::TurnEmotion;

/// Вероятности классов GigaAM-Emo, в сумме 1. Порядок выхода модели: angry, sad,
/// neutral, positive (id2name чекпойнта).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmotionProbs {
    pub angry: f32,
    pub sad: f32,
    pub neutral: f32,
    pub positive: f32,
}

impl EmotionProbs {
    /// Порог уверенности: ниже него у реплики нет эмоции. Подобран на тестовой части RESD
    /// (280 записей): при 0,7 метку получают 88 % записей, а метки кроме нейтральной верны
    /// в 83 % случаев (без порога в 78 %).
    pub const THRESHOLD: f32 = 0.7;

    pub fn from_output(probs: [f32; 4]) -> Self {
        let [angry, sad, neutral, positive] = probs;
        Self {
            angry,
            sad,
            neutral,
            positive,
        }
    }

    /// Среднее с весами, например по длине окна. None, если нечего усреднять или все
    /// веса нулевые.
    pub fn weighted_mean(items: &[(Self, f64)]) -> Option<Self> {
        let total: f64 = items.iter().map(|(_, w)| w).sum();
        if items.is_empty() || total <= 0.0 || !total.is_finite() {
            return None;
        }
        let mean = |field: fn(&Self) -> f32| {
            (items
                .iter()
                .map(|(p, w)| f64::from(field(p)) * w)
                .sum::<f64>()
                / total) as f32
        };
        Some(Self {
            angry: mean(|p| p.angry),
            sad: mean(|p| p.sad),
            neutral: mean(|p| p.neutral),
            positive: mean(|p| p.positive),
        })
    }

    /// Самый вероятный класс, если его вероятность не ниже `threshold`. Позитив модели
    /// показывается как радость, злость как гнев.
    pub fn emotion(&self, threshold: f32) -> Option<TurnEmotion> {
        let classes = [
            (self.angry, TurnEmotion::Anger),
            (self.sad, TurnEmotion::Sadness),
            (self.neutral, TurnEmotion::Neutral),
            (self.positive, TurnEmotion::Joy),
        ];
        let (best, emotion) = classes
            .into_iter()
            .fold((f32::NEG_INFINITY, None), |acc, (p, e)| {
                if p > acc.0 { (p, Some(e)) } else { acc }
            });
        emotion.filter(|_| best.is_finite() && best >= threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probs(p: [f32; 4]) -> EmotionProbs {
        EmotionProbs::from_output(p)
    }

    #[test]
    fn classes_follow_the_model_output_order() {
        assert_eq!(
            probs([0.8, 0.1, 0.05, 0.05]).emotion(0.7),
            Some(TurnEmotion::Anger)
        );
        assert_eq!(
            probs([0.1, 0.8, 0.05, 0.05]).emotion(0.7),
            Some(TurnEmotion::Sadness)
        );
        assert_eq!(
            probs([0.05, 0.1, 0.8, 0.05]).emotion(0.7),
            Some(TurnEmotion::Neutral)
        );
        assert_eq!(
            probs([0.05, 0.05, 0.1, 0.8]).emotion(0.7),
            Some(TurnEmotion::Joy)
        );
    }

    #[test]
    fn an_unsure_answer_gives_no_emotion() {
        assert_eq!(probs([0.4, 0.1, 0.3, 0.2]).emotion(0.7), None);
        assert_eq!(
            probs([0.7, 0.1, 0.1, 0.1]).emotion(0.7),
            Some(TurnEmotion::Anger)
        );
    }

    #[test]
    fn a_broken_output_gives_no_emotion() {
        assert_eq!(probs([f32::NAN; 4]).emotion(0.7), None);
    }

    #[test]
    fn the_mean_weights_windows_by_length() {
        let mean = EmotionProbs::weighted_mean(&[
            (probs([1.0, 0.0, 0.0, 0.0]), 3.0),
            (probs([0.0, 0.0, 1.0, 0.0]), 1.0),
        ])
        .unwrap();
        assert!((mean.angry - 0.75).abs() < 1e-6 && (mean.neutral - 0.25).abs() < 1e-6);
    }

    #[test]
    fn nothing_to_average_gives_none() {
        assert_eq!(EmotionProbs::weighted_mean(&[]), None);
        assert_eq!(
            EmotionProbs::weighted_mean(&[(probs([1.0, 0.0, 0.0, 0.0]), 0.0)]),
            None
        );
    }
}
