use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Перевод записи в WAV 16 кГц и выравнивание громкости.
    Prepare,
    /// Куски речи, распознавание, голоса: долгая часть, прогресс по звуку.
    Recognize,
    /// Нейросегментация, разделение по спикерам и сборка расшифровки.
    Voices,
}

impl Phase {
    /// Этап по проходу, который прислал Kotlin-плагин: "voices" значит нейросегментацию,
    /// остальное и отсутствие поля значат распознавание.
    pub fn of_engine(phase: Option<&str>) -> Self {
        match phase {
            Some("voices") => Phase::Voices,
            _ => Phase::Recognize,
        }
    }
}

/// Прогресс для интерфейса: этап и сколько секунд записи пройдено из скольких.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: Phase,
    pub done_sec: f64,
    pub total_sec: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_pass_becomes_interface_phase() {
        assert_eq!(Phase::of_engine(Some("voices")), Phase::Voices);
        assert_eq!(Phase::of_engine(Some("recognize")), Phase::Recognize);
        assert_eq!(Phase::of_engine(None), Phase::Recognize);
    }
}
