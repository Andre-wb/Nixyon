use nixyon_core::{Transcript, Turn};

use crate::Labels;

/// Имя спикера для вывода: своё, если задано, иначе «Спикер N» на языке интерфейса.
pub fn speaker_name(transcript: &Transcript, id: u32, labels: &Labels) -> String {
    transcript
        .speaker(id)
        .and_then(|s| s.name.clone())
        .unwrap_or_else(|| labels.speaker(id))
}

/// Имя спикера хода: с пометкой, если спикер под вопросом.
pub fn turn_speaker_name(transcript: &Transcript, turn: &Turn, labels: &Labels) -> String {
    let name = speaker_name(transcript, turn.speaker, labels);
    if turn.is_speaker_unsure() {
        labels.speaker_unsure(&name)
    } else {
        name
    }
}
