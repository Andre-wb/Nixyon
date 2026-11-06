use nixyon_core::{Speaker, Transcript};

use crate::clock::clock;
use crate::labels::Labels;
use crate::names::speaker_name;
use crate::txt::one_line;

/// Строки таблицы участников, как в Markdown: по убыванию времени речи, при равенстве
/// по номеру спикера; доля в целых процентах от суммы времени речи.
pub fn participant_rows(transcript: &Transcript, labels: &Labels) -> Vec<Vec<String>> {
    let total: f64 = transcript.speakers.iter().map(|s| s.talk_time_sec).sum();
    let mut speakers: Vec<&Speaker> = transcript.speakers.iter().collect();
    speakers.sort_by(|a, b| {
        b.talk_time_sec
            .total_cmp(&a.talk_time_sec)
            .then_with(|| a.id.cmp(&b.id))
    });
    speakers
        .into_iter()
        .map(|s| {
            let share = if total > 0.0 {
                (s.talk_time_sec / total * 100.0).round() as u32
            } else {
                0
            };
            vec![
                one_line(&speaker_name(transcript, s.id, labels)),
                clock(s.talk_time_sec),
                labels.percent(share),
            ]
        })
        .collect()
}
