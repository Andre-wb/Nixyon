use nixyon_metrics::WordEdit;
use serde::Serialize;
use serde::ser::{SerializeSeq, Serializer};

use crate::{ClipResult, ClipTag};

/// Строка файла клипов набора: итог клипа, метки для фильтра, выравнивание слов для
/// подсветки и номер записанного WAV, если звук клипа сохранён.
#[derive(Debug, Serialize)]
pub struct ClipLine<'a> {
    #[serde(flatten)]
    pub result: &'a ClipResult,
    pub tags: Vec<ClipTag>,
    /// Только у наборов с текстом: без эталона сравнивать не с чем.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edits: Option<Vec<EditCell>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<usize>,
}

/// Правка слова коротким массивом, чтобы файл на сотни тысяч клипов был меньше:
/// `["=", слово]`, `["~", эталон, распознано]`, `["-", пропущено]`, `["+", лишнее]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditCell(pub WordEdit);

impl Serialize for EditCell {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let parts: Vec<&str> = match &self.0 {
            WordEdit::Same(word) => vec!["=", word],
            WordEdit::Replaced {
                reference,
                hypothesis,
            } => vec!["~", reference, hypothesis],
            WordEdit::Missed(word) => vec!["-", word],
            WordEdit::Extra(word) => vec!["+", word],
        };
        let mut seq = serializer.serialize_seq(Some(parts.len()))?;
        for part in parts {
            seq.serialize_element(part)?;
        }
        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClipHypothesis;

    #[test]
    fn a_line_carries_the_result_tags_edits_and_audio_number() {
        let result = ClipResult::new(
            "a",
            "раз два",
            1.0,
            0.1,
            Ok(ClipHypothesis {
                text: "раз три лишнее".into(),
                speakers: 1,
            }),
        );
        let line = ClipLine {
            result: &result,
            tags: vec![ClipTag::Errors],
            edits: Some(vec![
                EditCell(WordEdit::Same("раз".into())),
                EditCell(WordEdit::Replaced {
                    reference: "два".into(),
                    hypothesis: "три".into(),
                }),
                EditCell(WordEdit::Extra("лишнее".into())),
                EditCell(WordEdit::Missed("четыре".into())),
            ]),
            audio: Some(3),
        };
        let value = serde_json::to_value(&line).unwrap();
        assert_eq!(value["id"], "a");
        assert_eq!(value["word_errors"], 2);
        assert_eq!(value["tags"], serde_json::json!(["errors"]));
        assert_eq!(
            value["edits"],
            serde_json::json!([
                ["=", "раз"],
                ["~", "два", "три"],
                ["+", "лишнее"],
                ["-", "четыре"]
            ])
        );
        assert_eq!(value["audio"], 3);
        let bare = ClipLine {
            result: &result,
            tags: Vec::new(),
            edits: None,
            audio: None,
        };
        let value = serde_json::to_value(&bare).unwrap();
        assert!(value.get("edits").is_none() && value.get("audio").is_none());
    }
}
