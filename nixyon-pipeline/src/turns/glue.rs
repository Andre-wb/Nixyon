use nixyon_core::Turn;

/// Соседние ходы одного спикера склеиваются, только если пауза между ними меньше
/// этого порога. Общий для конвейера и правок спикеров: иначе после слияния ходы,
/// между которыми была долгая тишина, станут одной огромной репликой.
pub const MAX_PAUSE_TO_GLUE_SEC: f64 = 1.0;

/// Склеивает соседние ходы одного спикера с паузой меньше порога. Ходы идут по времени.
/// Ходы с разными пометками не склеиваются: иначе пометка короткого места, например
/// «говорят одновременно», расползётся на всю длинную реплику.
pub fn glue_turns(turns: Vec<Turn>) -> Vec<Turn> {
    let mut glued: Vec<Turn> = Vec::with_capacity(turns.len());
    for turn in turns {
        match glued.last_mut() {
            Some(last)
                if last.speaker == turn.speaker
                    && turn.start - last.end < MAX_PAUSE_TO_GLUE_SEC
                    && same_flags(&last.flags, &turn.flags) =>
            {
                append(last, turn)
            }
            _ => glued.push(turn),
        }
    }
    glued
}

fn append(into: &mut Turn, next: Turn) {
    into.text = match (into.text.is_empty(), next.text.is_empty()) {
        (_, true) => std::mem::take(&mut into.text),
        (true, false) => next.text,
        (false, false) => format!("{} {}", into.text, next.text),
    };
    into.start = into.start.min(next.start);
    into.end = into.end.max(next.end);
    // Если у одного из ходов нет времени слов, у склеенного его тоже нет:
    // иначе слова перестанут совпадать с текстом.
    into.words = match (into.words.take(), next.words) {
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
        _ => None,
    };
}

fn same_flags(a: &[String], b: &[String]) -> bool {
    a.iter().all(|f| b.contains(f)) && b.iter().all(|f| a.contains(f))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{transcript, word};

    fn glued(turns: &[(u32, f64, f64, &str)]) -> Vec<Turn> {
        glue_turns(transcript(turns).turns)
    }

    #[test]
    fn pause_just_below_threshold_glues() {
        let t = glued(&[(1, 0.0, 1.0, "а"), (1, 1.999, 3.0, "б")]);
        assert_eq!(t.len(), 1);
        assert_eq!(
            (t[0].start, t[0].end, t[0].text.as_str()),
            (0.0, 3.0, "а б")
        );
    }

    #[test]
    fn pause_equal_to_threshold_or_longer_does_not_glue() {
        assert_eq!(glued(&[(1, 0.0, 1.0, "а"), (1, 2.0, 3.0, "б")]).len(), 2);
        assert_eq!(glued(&[(1, 0.0, 1.0, "а"), (1, 61.0, 62.0, "б")]).len(), 2);
    }

    #[test]
    fn overlapping_turns_of_one_speaker_glue() {
        let t = glued(&[(1, 0.0, 2.0, "а"), (1, 1.5, 3.0, "б")]);
        assert_eq!(t.len(), 1);
        assert_eq!((t[0].start, t[0].end), (0.0, 3.0));
    }

    #[test]
    fn different_speakers_never_glue() {
        assert_eq!(glued(&[(1, 0.0, 1.0, "а"), (2, 1.0, 2.0, "б")]).len(), 2);
    }

    #[test]
    fn chain_of_short_pauses_glues_into_one_turn() {
        let t = glued(&[(1, 0.0, 1.0, "а"), (1, 1.5, 2.0, "б"), (1, 2.5, 3.0, "в")]);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].text, "а б в");
    }

    #[test]
    fn word_timings_survive_only_if_both_turns_had_them() {
        let mut turns = transcript(&[(1, 0.0, 1.0, "а"), (1, 1.0, 2.0, "б")]).turns;
        turns[0].words = Some(vec![word("а", 0.0, 1.0)]);
        turns[1].words = Some(vec![word("б", 1.0, 2.0)]);
        let both = glue_turns(turns.clone());
        assert_eq!(
            both[0].words,
            Some(vec![word("а", 0.0, 1.0), word("б", 1.0, 2.0)])
        );

        turns[1].words = None;
        assert_eq!(glue_turns(turns)[0].words, None);
    }

    #[test]
    fn turns_with_the_same_flags_in_any_order_glue_and_keep_them() {
        let mut turns = transcript(&[(1, 0.0, 1.0, "а"), (1, 1.0, 2.0, "б")]).turns;
        turns[0].flags = vec!["clipping".into(), "overlap".into()];
        turns[1].flags = vec!["overlap".into(), "clipping".into()];
        let t = glue_turns(turns);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].flags, ["clipping", "overlap"]);
    }

    #[test]
    fn turns_with_different_flags_do_not_glue() {
        let mut turns =
            transcript(&[(1, 0.0, 1.0, "а"), (1, 1.0, 2.0, "б"), (1, 2.0, 3.0, "в")]).turns;
        turns[1].flags = vec!["overlap".into()];
        let t = glue_turns(turns);
        let shape: Vec<(&str, usize)> =
            t.iter().map(|x| (x.text.as_str(), x.flags.len())).collect();
        assert_eq!(shape, [("а", 0), ("б", 1), ("в", 0)]);
    }

    #[test]
    fn empty_text_does_not_add_extra_spaces() {
        let t = glued(&[(1, 0.0, 1.0, ""), (1, 1.0, 2.0, "б"), (1, 2.0, 3.0, "")]);
        assert_eq!(t[0].text, "б");
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(glue_turns(Vec::new()).is_empty());
    }
}
