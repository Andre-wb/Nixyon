use nixyon_core::Transcript;

/// Пересчитывает время речи, число ходов и слов у каждого спикера по ходам.
pub fn recompute_speaker_stats(transcript: &mut Transcript) {
    for speaker in &mut transcript.speakers {
        let mut talk_time = 0.0;
        let mut turns: u32 = 0;
        let mut words: u32 = 0;
        for turn in transcript.turns.iter().filter(|t| t.speaker == speaker.id) {
            talk_time += turn.duration();
            turns = turns.saturating_add(1);
            words = words.saturating_add(u32::try_from(turn.word_count()).unwrap_or(u32::MAX));
        }
        speaker.talk_time_sec = talk_time;
        speaker.turns = turns;
        speaker.words = words;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{transcript, word};

    #[test]
    fn sums_talk_time_turns_and_words_per_speaker() {
        let mut t = transcript(&[
            (1, 0.0, 2.0, "раз два"),
            (2, 2.0, 3.5, "три"),
            (1, 4.0, 5.0, "четыре пять шесть"),
        ]);
        recompute_speaker_stats(&mut t);
        let s1 = t.speaker(1).unwrap();
        assert_eq!((s1.talk_time_sec, s1.turns, s1.words), (3.0, 2, 5));
        let s2 = t.speaker(2).unwrap();
        assert_eq!((s2.talk_time_sec, s2.turns, s2.words), (1.5, 1, 1));
    }

    #[test]
    fn words_come_from_word_timings_when_present() {
        let mut t = transcript(&[(1, 0.0, 1.0, "текст из трёх")]);
        t.turns[0].words = Some(vec![word("текст", 0.0, 0.5), word("из", 0.5, 1.0)]);
        recompute_speaker_stats(&mut t);
        assert_eq!(t.speaker(1).unwrap().words, 2);
    }

    #[test]
    fn speaker_without_turns_gets_zeros() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а")]);
        t.speakers[0].talk_time_sec = 9.0;
        t.speakers[0].turns = 9;
        t.speakers[0].words = 9;
        t.turns.clear();
        recompute_speaker_stats(&mut t);
        let s = t.speaker(1).unwrap();
        assert_eq!((s.talk_time_sec, s.turns, s.words), (0.0, 0, 0));
    }
}
