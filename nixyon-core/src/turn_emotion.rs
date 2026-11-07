use serde::{Deserialize, Serialize};

/// Эмоция реплики: модель ставит нейтральную, радость, грусть или гнев, остальные
/// выбирает человек вручную.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TurnEmotion {
    Neutral,
    Joy,
    Sadness,
    Anger,
    Fear,
    Surprise,
    Disgust,
}

impl TurnEmotion {
    /// Все эмоции в порядке списка выбора.
    pub const ALL: [Self; 7] = [
        Self::Neutral,
        Self::Joy,
        Self::Sadness,
        Self::Anger,
        Self::Fear,
        Self::Surprise,
        Self::Disgust,
    ];

    /// Нейтральная реплика в текстовых форматах экспорта не помечается.
    pub fn is_neutral(self) -> bool {
        self == Self::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emotions_are_written_in_snake_case() {
        let names: Vec<String> = TurnEmotion::ALL
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "\"neutral\"",
                "\"joy\"",
                "\"sadness\"",
                "\"anger\"",
                "\"fear\"",
                "\"surprise\"",
                "\"disgust\""
            ]
        );
    }

    #[test]
    fn an_unknown_emotion_is_refused() {
        assert!(serde_json::from_str::<TurnEmotion>("\"boredom\"").is_err());
    }

    #[test]
    fn only_neutral_is_neutral() {
        let neutral: Vec<_> = TurnEmotion::ALL.iter().filter(|e| e.is_neutral()).collect();
        assert_eq!(neutral, [&TurnEmotion::Neutral]);
    }
}
