mod interruptions;
mod speaker_summary;

use nixyon_core::Transcript;

pub use speaker_summary::SpeakerSummary;

use interruptions::interruptions;

/// Ход дольше этого считается монологом. Порог не подбирался замером.
pub const MONOLOGUE_SEC: f64 = 120.0;

/// Статистика встречи для экрана «Статистика».
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingStats {
    /// В порядке спикеров расшифровки.
    pub speakers: Vec<SpeakerSummary>,
    /// Все перебивания, включая те, где перебившего не определить.
    pub interruptions: u32,
    /// Сумма времени речи всех спикеров.
    pub talk_time_sec: f64,
}

/// Считает статистику по ходам; поля статистики у спикеров не читаются.
pub fn meeting_stats(transcript: &Transcript) -> MeetingStats {
    let events = interruptions(&transcript.turns);
    let speakers: Vec<SpeakerSummary> = transcript
        .speakers
        .iter()
        .map(|s| {
            let by_speaker = events.iter().filter(|e| e.by == Some(s.id)).count();
            SpeakerSummary::from_turns(
                s.id,
                transcript.turns.iter().filter(|t| t.speaker == s.id),
                u32::try_from(by_speaker).unwrap_or(u32::MAX),
            )
        })
        .collect();
    MeetingStats {
        talk_time_sec: speakers.iter().map(|s| s.talk_time_sec).sum(),
        interruptions: u32::try_from(events.len()).unwrap_or(u32::MAX),
        speakers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    #[test]
    fn summaries_follow_speaker_order_and_count_interruptions() {
        let t = transcript(&[(1, 0.0, 5.0, "раз два"), (2, 4.0, 6.0, "три")]);
        let stats = meeting_stats(&t);
        assert_eq!(
            stats.speakers.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(stats.interruptions, 1);
        assert_eq!(stats.speakers[1].interruptions, 1);
        assert_eq!(stats.speakers[0].interruptions, 0);
        assert_eq!(stats.talk_time_sec, 7.0);
    }

    #[test]
    fn stale_speaker_fields_do_not_affect_stats() {
        let mut t = transcript(&[(1, 0.0, 2.0, "раз два")]);
        t.speakers[0].talk_time_sec = 99.0;
        t.speakers[0].words = 99;
        let stats = meeting_stats(&t);
        assert_eq!(
            (stats.speakers[0].talk_time_sec, stats.speakers[0].words),
            (2.0, 2)
        );
    }
}
