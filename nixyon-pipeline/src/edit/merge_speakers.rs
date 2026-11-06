use nixyon_core::Transcript;

use crate::turns::glue_turns;
use crate::{EditError, recompute_speaker_stats};

/// Сливает спикера `absorb` в `keep`: его ходы переходят к `keep`, сам он удаляется,
/// соседние ходы одного спикера с короткой паузой склеиваются, ходы перенумеровываются,
/// статистика пересчитывается. При ошибке расшифровка не меняется.
pub fn merge_speakers(
    transcript: &mut Transcript,
    keep: u32,
    absorb: u32,
) -> Result<(), EditError> {
    if keep == absorb {
        return Err(EditError::SameSpeaker);
    }
    transcript
        .speaker(keep)
        .ok_or(EditError::UnknownSpeaker(keep))?;
    let absorbed = transcript
        .speaker(absorb)
        .ok_or(EditError::UnknownSpeaker(absorb))?;
    let (absorbed_name, absorbed_age, absorbed_gender) =
        (absorbed.name.clone(), absorbed.age, absorbed.gender);

    if let Some(kept) = transcript.speaker_mut(keep) {
        if kept.name.is_none() {
            kept.name = absorbed_name;
        }
        // Метки голоса остаются, только если у обоих они одни: иначе неизвестно, чей голос
        // теперь у спикера.
        if kept.age != absorbed_age {
            kept.age = None;
        }
        if kept.gender != absorbed_gender {
            kept.gender = None;
        }
    }
    transcript.speakers.retain(|s| s.id != absorb);
    for turn in &mut transcript.turns {
        if turn.speaker == absorb {
            turn.speaker = keep;
        }
    }
    transcript.turns = glue_turns(std::mem::take(&mut transcript.turns));
    for (turn, id) in transcript.turns.iter_mut().zip(1..) {
        turn.id = id;
    }
    recompute_speaker_stats(transcript);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;
    use nixyon_core::{SpeakerAge, SpeakerGender};

    #[test]
    fn the_age_stays_only_when_both_speakers_have_the_same_one() {
        let cases = [
            (
                Some(SpeakerAge::Child),
                Some(SpeakerAge::Child),
                Some(SpeakerAge::Child),
            ),
            (Some(SpeakerAge::Child), Some(SpeakerAge::Adult), None),
            (Some(SpeakerAge::Adult), None, None),
            (None, Some(SpeakerAge::Adult), None),
        ];
        for (kept, absorbed, merged) in cases {
            let mut t = transcript(&[(1, 0.0, 1.0, "а"), (2, 1.0, 2.0, "б")]);
            t.speaker_mut(1).unwrap().age = kept;
            t.speaker_mut(2).unwrap().age = absorbed;
            merge_speakers(&mut t, 1, 2).unwrap();
            assert_eq!(t.speaker(1).unwrap().age, merged, "{kept:?} и {absorbed:?}");
        }
    }

    #[test]
    fn the_gender_stays_only_when_both_speakers_have_the_same_one() {
        let cases = [
            (
                Some(SpeakerGender::Male),
                Some(SpeakerGender::Male),
                Some(SpeakerGender::Male),
            ),
            (Some(SpeakerGender::Male), Some(SpeakerGender::Female), None),
            (None, Some(SpeakerGender::Female), None),
        ];
        for (kept, absorbed, merged) in cases {
            let mut t = transcript(&[(1, 0.0, 1.0, "а"), (2, 1.0, 2.0, "б")]);
            t.speaker_mut(1).unwrap().gender = kept;
            t.speaker_mut(2).unwrap().gender = absorbed;
            merge_speakers(&mut t, 1, 2).unwrap();
            assert_eq!(
                t.speaker(1).unwrap().gender,
                merged,
                "{kept:?} и {absorbed:?}"
            );
        }
    }

    #[test]
    fn absorbed_speaker_turns_move_to_kept_and_speaker_disappears() {
        let mut t = transcript(&[
            (1, 0.0, 1.0, "а"),
            (3, 1.0, 2.0, "б"),
            (2, 2.0, 3.0, "в"),
            (3, 3.0, 4.0, "г"),
        ]);
        merge_speakers(&mut t, 1, 3).unwrap();
        assert!(t.speaker(3).is_none());
        let speakers: Vec<u32> = t.turns.iter().map(|x| x.speaker).collect();
        assert_eq!(speakers, [1, 2, 1]);
    }

    #[test]
    fn neighbouring_turns_with_short_pause_are_glued() {
        let mut t = transcript(&[
            (1, 0.0, 1.0, "раз"),
            (2, 1.2, 2.0, "два"),
            (3, 2.5, 3.0, "три"),
        ]);
        merge_speakers(&mut t, 1, 2).unwrap();
        assert_eq!(t.turns.len(), 2);
        assert_eq!(t.turns[0].text, "раз два");
        assert_eq!((t.turns[0].start, t.turns[0].end), (0.0, 2.0));
        assert_eq!(t.turns[1].speaker, 3);
    }

    #[test]
    fn neighbouring_turns_with_long_pause_stay_apart() {
        let mut t = transcript(&[(1, 0.0, 1.0, "раз"), (2, 60.0, 61.0, "два")]);
        merge_speakers(&mut t, 1, 2).unwrap();
        assert_eq!(t.turns.len(), 2);
        assert!(t.turns.iter().all(|x| x.speaker == 1));
        assert_eq!(t.speaker(1).unwrap().turns, 2);
    }

    #[test]
    fn turns_are_renumbered_but_speaker_ids_are_kept() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а"), (2, 1.0, 2.0, "б"), (3, 2.0, 3.0, "в")]);
        merge_speakers(&mut t, 1, 2).unwrap();
        let ids: Vec<u32> = t.turns.iter().map(|x| x.id).collect();
        assert_eq!(ids, [1, 2]);
        let speaker_ids: Vec<u32> = t.speakers.iter().map(|s| s.id).collect();
        assert_eq!(speaker_ids, [1, 3]);
    }

    #[test]
    fn stats_are_recomputed() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а б"), (2, 1.0, 3.0, "в")]);
        merge_speakers(&mut t, 2, 1).unwrap();
        let s = t.speaker(2).unwrap();
        assert_eq!((s.talk_time_sec, s.turns, s.words), (3.0, 1, 3));
    }

    #[test]
    fn kept_name_wins_and_missing_name_is_taken_from_absorbed() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а"), (2, 1.0, 2.0, "б"), (3, 2.0, 3.0, "в")]);
        t.speaker_mut(2).unwrap().name = Some("Борис".into());
        merge_speakers(&mut t, 1, 2).unwrap();
        assert_eq!(t.speaker(1).unwrap().name.as_deref(), Some("Борис"));

        t.speaker_mut(3).unwrap().name = Some("Анна".into());
        merge_speakers(&mut t, 1, 3).unwrap();
        assert_eq!(t.speaker(1).unwrap().name.as_deref(), Some("Борис"));
    }

    #[test]
    fn same_or_unknown_speaker_is_an_error_and_nothing_changes() {
        let mut t = transcript(&[(1, 0.0, 1.0, "а"), (2, 1.0, 2.0, "б")]);
        let before = t.clone();
        assert_eq!(merge_speakers(&mut t, 1, 1), Err(EditError::SameSpeaker));
        assert_eq!(
            merge_speakers(&mut t, 1, 9),
            Err(EditError::UnknownSpeaker(9))
        );
        assert_eq!(
            merge_speakers(&mut t, 9, 1),
            Err(EditError::UnknownSpeaker(9))
        );
        assert_eq!(t, before);
    }
}
