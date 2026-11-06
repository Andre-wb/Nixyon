use nixyon_pipeline::{MONOLOGUE_SEC, MeetingStats, SpeakerSummary};
use serde::Serialize;

/// Статистика спикера для интерфейса. Имя интерфейс берёт из расшифровки.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerStatsView {
    pub id: u32,
    pub talk_time_sec: f64,
    pub turns: u32,
    pub words: u32,
    pub words_per_minute: Option<f64>,
    pub interruptions: u32,
    pub monologues: u32,
    pub longest_turn_sec: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    pub speakers: Vec<SpeakerStatsView>,
    pub interruptions: u32,
    pub talk_time_sec: f64,
    /// Порог монолога: интерфейс пишет его в подписи.
    pub monologue_sec: f64,
}

impl From<&SpeakerSummary> for SpeakerStatsView {
    fn from(s: &SpeakerSummary) -> Self {
        SpeakerStatsView {
            id: s.id,
            talk_time_sec: s.talk_time_sec,
            turns: s.turns,
            words: s.words,
            words_per_minute: s.words_per_minute,
            interruptions: s.interruptions,
            monologues: s.monologues,
            longest_turn_sec: s.longest_turn_sec,
        }
    }
}

impl From<&MeetingStats> for StatsView {
    fn from(m: &MeetingStats) -> Self {
        StatsView {
            speakers: m.speakers.iter().map(SpeakerStatsView::from).collect(),
            interruptions: m.interruptions,
            talk_time_sec: m.talk_time_sec,
            monologue_sec: MONOLOGUE_SEC,
        }
    }
}
