use nixyon_core::{OVERLAP_FLAG, Transcript};
use nixyon_metrics::{NamedSpan, SpeakerSpan, Utterance};

pub fn hypothesis_spans(transcript: &Transcript) -> Vec<SpeakerSpan> {
    transcript
        .turns
        .iter()
        .map(|t| SpeakerSpan {
            speaker: t.speaker,
            start: t.start,
            end: t.end,
        })
        .collect()
}

/// Ходы расшифровки по времени.
pub fn hypothesis_utterances(transcript: &Transcript) -> Vec<Utterance> {
    let mut turns: Vec<_> = transcript.turns.iter().collect();
    turns.sort_by(|a, b| a.start.total_cmp(&b.start));
    turns
        .into_iter()
        .map(|t| Utterance {
            speaker: t.speaker,
            text: t.text.clone(),
        })
        .collect()
}

pub fn hypothesis_text(transcript: &Transcript) -> String {
    hypothesis_utterances(transcript)
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Время ходов с пометкой «говорят одновременно».
pub fn hypothesis_overlaps(transcript: &Transcript) -> Vec<(f64, f64)> {
    transcript
        .turns
        .iter()
        .filter(|t| t.flags.iter().any(|f| f == OVERLAP_FLAG))
        .map(|t| (t.start, t.end))
        .collect()
}

/// Ходы с именем, которое спикер получил по образцу (None: имени нет).
pub fn hypothesis_named_spans(transcript: &Transcript) -> Vec<NamedSpan> {
    transcript
        .turns
        .iter()
        .map(|t| NamedSpan {
            speaker: t.speaker,
            name: transcript.speaker(t.speaker).and_then(|s| s.name.clone()),
            start: t.start,
            end: t.end,
        })
        .collect()
}
