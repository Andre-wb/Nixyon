use nixyon_core::Turn;

use super::MONOLOGUE_SEC;

/// Статистика одного спикера.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerSummary {
    pub id: u32,
    pub talk_time_sec: f64,
    pub turns: u32,
    pub words: u32,
    /// Слов в минуту времени речи; None, если спикер не говорил.
    pub words_per_minute: Option<f64>,
    /// Сколько раз этот спикер перебил других.
    pub interruptions: u32,
    /// Ходов дольше `MONOLOGUE_SEC`.
    pub monologues: u32,
    pub longest_turn_sec: f64,
}

impl SpeakerSummary {
    pub(crate) fn from_turns<'a>(
        id: u32,
        turns: impl Iterator<Item = &'a Turn>,
        interruptions: u32,
    ) -> Self {
        let mut s = SpeakerSummary {
            id,
            talk_time_sec: 0.0,
            turns: 0,
            words: 0,
            words_per_minute: None,
            interruptions,
            monologues: 0,
            longest_turn_sec: 0.0,
        };
        for turn in turns {
            let duration = turn.duration();
            s.talk_time_sec += duration;
            s.turns = s.turns.saturating_add(1);
            s.words = s
                .words
                .saturating_add(u32::try_from(turn.word_count()).unwrap_or(u32::MAX));
            if duration > MONOLOGUE_SEC {
                s.monologues = s.monologues.saturating_add(1);
            }
            s.longest_turn_sec = s.longest_turn_sec.max(duration);
        }
        if s.talk_time_sec > 0.0 {
            s.words_per_minute = Some(f64::from(s.words) * 60.0 / s.talk_time_sec);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    fn summary(turns: &[(u32, f64, f64, &str)]) -> SpeakerSummary {
        let t = transcript(turns);
        SpeakerSummary::from_turns(1, t.turns.iter(), 0)
    }

    #[test]
    fn pace_is_words_per_minute_of_talk_time() {
        let s = summary(&[
            (1, 0.0, 30.0, "раз два три"),
            (1, 40.0, 70.0, "четыре пять шесть"),
        ]);
        assert_eq!((s.talk_time_sec, s.turns, s.words), (60.0, 2, 6));
        assert_eq!(s.words_per_minute, Some(6.0));
    }

    #[test]
    fn speaker_without_speech_has_no_pace() {
        let s = summary(&[]);
        assert_eq!(s.words_per_minute, None);
        assert_eq!((s.turns, s.monologues, s.longest_turn_sec), (0, 0, 0.0));
    }

    #[test]
    fn only_turns_longer_than_threshold_are_monologues() {
        let s = summary(&[
            (1, 0.0, MONOLOGUE_SEC, "ровно порог"),
            (1, 200.0, 200.0 + MONOLOGUE_SEC + 1.0, "дольше порога"),
        ]);
        assert_eq!(s.monologues, 1);
        assert_eq!(s.longest_turn_sec, MONOLOGUE_SEC + 1.0);
    }
}
