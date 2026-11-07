use nixyon_core::{Turn, TurnEmotion};

use crate::EmotionLabels;

/// Служебные надписи экспорта на языке интерфейса. Шаблоны с подстановками:
/// `speaker_default` содержит `{n}`, `percent` содержит `{value}`, `speaker_unsure`
/// содержит `{name}`.
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
    /// Имя спикера, который под вопросом: голос мало похож на него.
    pub speaker_unsure: String,
    /// Пометка неразборчивой речи: вместо текста или после распознанной части.
    pub unclear: String,
    pub emotions: EmotionLabels,
}

impl Labels {
    pub fn speaker(&self, n: u32) -> String {
        self.speaker_default.replace("{n}", &n.to_string())
    }

    pub fn speaker_unsure(&self, name: &str) -> String {
        self.speaker_unsure.replace("{name}", name)
    }

    /// Текст хода для вывода. Ход без текста (неразборчивая речь или второй говорящий в
    /// перекрытии) получает пометку вместо текста, ход, где распознана только часть,
    /// пометку после текста.
    pub fn turn_text(&self, turn: &Turn) -> String {
        let text = turn.text.trim();
        match (turn.is_unclear(), text.is_empty()) {
            (_, true) => self.unclear.clone(),
            (false, false) => turn.text.clone(),
            (true, false) => format!("{text} {}", self.unclear),
        }
    }

    pub fn percent(&self, value: u32) -> String {
        self.percent.replace("{value}", &value.to_string())
    }

    /// Подпись эмоции реплики; None для нейтральной и для реплики без эмоции.
    pub fn emotion(&self, emotion: Option<TurnEmotion>) -> Option<&str> {
        self.emotions.get(emotion)
    }
}
