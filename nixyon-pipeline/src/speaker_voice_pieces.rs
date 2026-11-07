use std::collections::BTreeMap;

use nixyon_core::{OVERLAP_FLAG, Transcript};

/// Сколько речи спикера слушает модель «ребёнок или взрослый»: при обучении у говорящего
/// тоже было до 30 клипов, а больше речи почти не меняет среднее, но удлиняет разбор.
pub const VOICE_PIECES_MAX_SEC: f64 = 30.0;
/// Реплика короче не берётся: по долям секунды голос оценивается плохо.
pub const VOICE_PIECE_MIN_SEC: f64 = 1.0;

/// Куски речи одного спикера: начало и конец в секундах, по порядку в записи.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerPieces {
    pub speaker: u32,
    pub pieces: Vec<(f64, f64)>,
}

/// Для каждого спикера самые длинные реплики, всего не больше `max_sec` секунд; последняя
/// обрезается по остатку. Реплики, где говорят сразу несколько, с неуверенным спикером и
/// короче `VOICE_PIECE_MIN_SEC` не берутся: там голос не одного спикера, может быть
/// чужим или его слишком мало.
/// Спикер без подходящих реплик не попадает в ответ. Порядок по номеру спикера.
pub fn speaker_voice_pieces(transcript: &Transcript, max_sec: f64) -> Vec<SpeakerPieces> {
    let mut turns: BTreeMap<u32, Vec<(f64, f64, u32)>> = BTreeMap::new();
    for t in &transcript.turns {
        if t.duration() >= VOICE_PIECE_MIN_SEC
            && !t.flags.iter().any(|f| f == OVERLAP_FLAG)
            && !t.is_speaker_unsure()
        {
            turns
                .entry(t.speaker)
                .or_default()
                .push((t.start, t.end, t.id));
        }
    }
    turns
        .into_iter()
        .filter_map(|(speaker, mut list)| {
            // Длинные первыми; при равной длине раньше идёт меньший номер реплики.
            list.sort_by(|a, b| (b.1 - b.0).total_cmp(&(a.1 - a.0)).then(a.2.cmp(&b.2)));
            let mut left = max_sec;
            let mut pieces = Vec::new();
            for (start, end, _) in list {
                if left < VOICE_PIECE_MIN_SEC {
                    break;
                }
                let take = (end - start).min(left);
                pieces.push((start, start + take));
                left -= take;
            }
            pieces.sort_by(|a, b| a.0.total_cmp(&b.0));
            (!pieces.is_empty()).then_some(SpeakerPieces { speaker, pieces })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    #[test]
    fn the_longest_turns_are_taken_up_to_the_limit_and_the_last_is_cut() {
        let t = transcript(&[
            (1, 0.0, 5.0, "а"),
            (1, 5.0, 25.0, "б"),
            (1, 25.0, 40.0, "в"),
            (1, 40.0, 42.0, "г"),
        ]);
        let got = speaker_voice_pieces(&t, 30.0);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].pieces, [(5.0, 25.0), (25.0, 35.0)]);
    }

    #[test]
    fn short_overlapping_and_unsure_turns_are_left_out() {
        let mut t = transcript(&[
            (1, 0.0, 0.5, "а"),
            (1, 1.0, 4.0, "б"),
            (2, 4.0, 9.0, "в"),
            (2, 9.0, 12.0, "г"),
        ]);
        t.turns[1].flags.push(OVERLAP_FLAG.into());
        t.turns[3]
            .flags
            .push(nixyon_core::SPEAKER_UNSURE_FLAG.into());
        let got = speaker_voice_pieces(&t, 30.0);
        assert_eq!(
            got,
            [SpeakerPieces {
                speaker: 2,
                pieces: vec![(4.0, 9.0)]
            }]
        );
    }

    #[test]
    fn a_remainder_shorter_than_a_second_is_not_taken() {
        let t = transcript(&[(1, 0.0, 29.5, "а"), (1, 30.0, 40.0, "б")]);
        let got = speaker_voice_pieces(&t, 30.0);
        assert_eq!(got[0].pieces, [(0.0, 29.5)]);
    }

    #[test]
    fn each_speaker_gets_its_own_pieces_in_order_of_the_recording() {
        let t = transcript(&[(2, 0.0, 3.0, "а"), (1, 3.0, 5.0, "б"), (2, 5.0, 9.0, "в")]);
        let got = speaker_voice_pieces(&t, 30.0);
        assert_eq!(got[0].speaker, 1);
        assert_eq!(got[1].pieces, [(0.0, 3.0), (5.0, 9.0)]);
    }
}
