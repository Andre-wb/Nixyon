use nixyon_core::TurnEmotion;

use crate::EmotionLabels;

/// Служебные надписи экспорта на языке интерфейса. Шаблоны с подстановками:
/// `speaker_default` содержит `{n}`, `percent` содержит `{value}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Labels {
    pub title: String,
    pub recorded: String,
    pub duration: String,
    pub participants: String,
    pub participant: String,
    pub talk_time: String,
    pub share: String,
    pub percent: String,
    pub speaker_default: String,
    pub emotions: EmotionLabels,
}

impl Labels {
    pub fn speaker(&self, n: u32) -> String {
        self.speaker_default.replace("{n}", &n.to_string())
    }

    pub fn percent(&self, value: u32) -> String {
        self.percent.replace("{value}", &value.to_string())
    }

    /// Подпись эмоции реплики; None для нейтральной и для реплики без эмоции.
    pub fn emotion(&self, emotion: Option<TurnEmotion>) -> Option<&str> {
        self.emotions.get(emotion)
    }
}
