use nixyon_core::Transcript;

use crate::clock::clock;
use crate::labels::Labels;
use crate::names::speaker_name;

/// Простой текст: заголовок со сводкой, затем ходы блоками «имя, время» и текст.
pub fn render_txt(transcript: &Transcript, labels: &Labels, recorded_at: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str(&labels.title);
    out.push('\n');
    if let Some(when) = recorded_at {
        out.push_str(&format!("{}: {}\n", labels.recorded, one_line(when)));
    }
    out.push_str(&format!(
        "{}: {}\n",
        labels.duration,
        clock(transcript.audio.duration_sec)
    ));
    out.push_str(&format!(
        "{}: {}\n",
        labels.participants,
        transcript.speakers.len()
    ));
    for turn in &transcript.turns {
        let emotion = labels
            .emotion(turn.emotion)
            .map(|e| format!(", {}", one_line(e)))
            .unwrap_or_default();
        out.push_str(&format!(
            "\n{}, {}{}\n{}\n",
            one_line(&speaker_name(transcript, turn.speaker, labels)),
            clock(turn.start),
            emotion,
            one_line(&turn.text)
        ));
    }
    out
}

/// Перевод строки внутри имени или текста сломал бы раскладку блоков.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
