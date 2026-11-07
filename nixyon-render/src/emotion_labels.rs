use nixyon_core::TurnEmotion;

/// Подписи эмоций в экспорте на языке интерфейса. У нейтральной подписи нет: в
/// экспорте она не пишется.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmotionLabels {
    pub joy: String,
    pub sadness: String,
    pub anger: String,
    pub fear: String,
    pub surprise: String,
    pub disgust: String,
}

impl EmotionLabels {
    /// Подпись эмоции реплики; None для нейтральной и для реплики без эмоции.
    pub fn get(&self, emotion: Option<TurnEmotion>) -> Option<&str> {
        let label = match emotion? {
            TurnEmotion::Neutral => return None,
            TurnEmotion::Joy => &self.joy,
            TurnEmotion::Sadness => &self.sadness,
            TurnEmotion::Anger => &self.anger,
            TurnEmotion::Fear => &self.fear,
            TurnEmotion::Surprise => &self.surprise,
            TurnEmotion::Disgust => &self.disgust,
        };
        Some(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> EmotionLabels {
        EmotionLabels {
            joy: "радость".into(),
            sadness: "грусть".into(),
            anger: "гнев".into(),
            fear: "страх".into(),
            surprise: "удивление".into(),
            disgust: "отвращение".into(),
        }
    }

    #[test]
    fn every_emotion_but_neutral_has_its_own_label() {
        let l = labels();
        let got: Vec<Option<&str>> = TurnEmotion::ALL.iter().map(|e| l.get(Some(*e))).collect();
        assert_eq!(
            got,
            [
                None,
                Some("радость"),
                Some("грусть"),
                Some("гнев"),
                Some("страх"),
                Some("удивление"),
                Some("отвращение")
            ]
        );
    }

    #[test]
    fn a_turn_without_emotion_has_no_label() {
        assert_eq!(labels().get(None), None);
    }
}
