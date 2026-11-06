use nixyon_metrics::{NamedSpan, SpeakerSpan, Utterance};
use serde::Deserialize;

use crate::TruthTurn;

/// Разметка записи из `<имя>.truth.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Truth {
    pub speakers: u32,
    pub turns: Vec<TruthTurn>,
}

impl Truth {
    /// Читает разметку; реплики упорядочиваются по времени.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut truth: Self = serde_json::from_str(json)?;
        truth.turns.sort_by(|a, b| a.start.total_cmp(&b.start));
        Ok(truth)
    }

    pub fn spans(&self) -> Vec<SpeakerSpan> {
        self.turns
            .iter()
            .map(|t| SpeakerSpan {
                speaker: t.speaker,
                start: t.start,
                end: t.end,
            })
            .collect()
    }

    /// Реплики с именем голоса (у реплики без поля `voice` имени нет).
    pub fn named_spans(&self) -> Vec<NamedSpan> {
        self.turns
            .iter()
            .map(|t| NamedSpan {
                speaker: t.speaker,
                name: t.voice.clone(),
                start: t.start,
                end: t.end,
            })
            .collect()
    }

    /// Реплики с текстом; None, если хоть у одной реплики текста нет.
    pub fn utterances(&self) -> Option<Vec<Utterance>> {
        self.turns
            .iter()
            .map(|t| {
                Some(Utterance {
                    speaker: t.speaker,
                    text: t.text.clone()?,
                })
            })
            .collect()
    }

    /// Весь текст по порядку реплик; None, если хоть у одной реплики текста нет.
    pub fn text(&self) -> Option<String> {
        let utterances = self.utterances()?;
        Some(
            utterances
                .iter()
                .map(|u| u.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_are_sorted_voices_are_read_and_extra_fields_are_ignored() {
        let truth = Truth::from_json(
            r#"{"speakers": 2, "turns": [
                {"speaker": 2, "start": 3.0, "end": 4.0, "text": "второй", "voice": "b"},
                {"speaker": 1, "start": 0.5, "end": 2.0, "text": "первый", "source": "x.wav"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(truth.text().as_deref(), Some("первый второй"));
        let named = truth.named_spans();
        let names: Vec<Option<&str>> = named.iter().map(|n| n.name.as_deref()).collect();
        assert_eq!(names, [None, Some("b")]);
        assert_eq!(
            truth.spans()[0],
            SpeakerSpan {
                speaker: 1,
                start: 0.5,
                end: 2.0
            }
        );
    }

    #[test]
    fn missing_text_in_any_turn_means_no_text_reference() {
        let truth = Truth::from_json(
            r#"{"speakers": 1, "turns": [
                {"speaker": 1, "start": 0.0, "end": 1.0, "text": "есть"},
                {"speaker": 1, "start": 1.0, "end": 2.0}
            ]}"#,
        )
        .unwrap();
        assert_eq!(truth.utterances(), None);
        assert_eq!(truth.text(), None);
        assert_eq!(truth.spans().len(), 2);
    }
}
