use nixyon_core::Transcript;

use crate::Labels;

/// Имя спикера для вывода: своё, если задано, иначе «Спикер N» на языке интерфейса.
pub fn speaker_name(transcript: &Transcript, id: u32, labels: &Labels) -> String {
    transcript
        .speaker(id)
        .and_then(|s| s.name.clone())
        .unwrap_or_else(|| labels.speaker(id))
}
