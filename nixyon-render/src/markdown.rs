use nixyon_core::{Speaker, Transcript};

use crate::clock::clock;
use crate::labels::Labels;
use crate::names::speaker_name;
use crate::txt::one_line;

/// Markdown: заголовок, сводка списком, таблица участников по времени речи, затем ходы.
pub fn render_markdown(
    transcript: &Transcript,
    labels: &Labels,
    recorded_at: Option<&str>,
) -> String {
    let mut out = format!("# {}\n\n", escape(&labels.title));
    if let Some(when) = recorded_at {
        out.push_str(&format!("- {}: {}\n", labels.recorded, escape(when)));
    }
    out.push_str(&format!(
        "- {}: {}\n- {}: {}\n",
        labels.duration,
        clock(transcript.audio.duration_sec),
        labels.participants,
        transcript.speakers.len()
    ));
    if !transcript.speakers.is_empty() {
        out.push_str(&format!(
            "\n| {} | {} | {} |\n|---|---:|---:|\n",
            escape(&labels.participant),
            escape(&labels.talk_time),
            escape(&labels.share)
        ));
        let total: f64 = transcript.speakers.iter().map(|s| s.talk_time_sec).sum();
        let mut speakers: Vec<&Speaker> = transcript.speakers.iter().collect();
        speakers.sort_by(|a, b| {
            b.talk_time_sec
                .total_cmp(&a.talk_time_sec)
                .then_with(|| a.id.cmp(&b.id))
        });
        for s in speakers {
            let share = if total > 0.0 {
                (s.talk_time_sec / total * 100.0).round() as u32
            } else {
                0
            };
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                escape(&speaker_name(transcript, s.id, labels)),
                clock(s.talk_time_sec),
                labels.percent(share)
            ));
        }
    }
    for turn in &transcript.turns {
        out.push_str(&format!(
            "\n**{}**, {}\n\n{}\n",
            escape(&speaker_name(transcript, turn.speaker, labels)),
            clock(turn.start),
            escape(&turn.text)
        ));
    }
    out
}

// Имена и речь людей не должны превращаться в разметку: «# план» не заголовок,
// «*да*» не курсив, «|» не ломает таблицу, «1. пункт» не список.
fn escape(text: &str) -> String {
    let line = one_line(text);
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        if "\\`*_{}[]<>()#+-!|~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    let digits = out.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && out[digits..].starts_with('.') {
        out.insert(digits, '\\');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn speech_never_becomes_markup() {
        assert_eq!(escape("# план"), "\\# план");
        assert_eq!(escape("*да* | нет"), "\\*да\\* \\| нет");
        assert_eq!(escape("1. пункт"), "1\\. пункт");
        assert_eq!(escape("- минус\nстрока"), "\\- минус строка");
        assert_eq!(escape("обычный текст 2.5"), "обычный текст 2.5");
    }
}
