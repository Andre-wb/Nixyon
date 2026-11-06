use std::collections::HashMap;

use nixyon_core::{OVERLAP_FLAG, Turn};

// Погрешность сравнения времени: части одного куска стыкуются встык, и после
// арифметики с f64 конец может оказаться чуть позже начала следующей части.
const TIME_EPS: f64 = 1e-6;

/// Перебивание. `by`: кто перебил, если это можно определить.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Interruption {
    pub by: Option<u32>,
}

/// Перебивание бывает двух видов. Ход начался раньше конца хода другого спикера:
/// перебил спикер нового хода. Подряд идущие ходы с пометкой перекрытия: одно
/// перебивание на всю серию, перебил первый спикер внутри серии или в ходе сразу после
/// неё, который не говорил перед серией. Если перед серией никто не говорил или все
/// ходы вокруг у одного спикера, перебившего не определить.
pub(crate) fn interruptions(turns: &[Turn]) -> Vec<Interruption> {
    let mut ordered: Vec<&Turn> = turns.iter().collect();
    ordered.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.id.cmp(&b.id)));

    let mut events = Vec::new();
    let mut last_end: HashMap<u32, f64> = HashMap::new();
    let mut i = 0;
    while i < ordered.len() {
        if is_overlap(ordered[i]) {
            let run_end = ordered[i..]
                .iter()
                .position(|t| !is_overlap(t))
                .map_or(ordered.len(), |n| i + n);
            let before = i.checked_sub(1).map(|p| ordered[p].speaker);
            let by = before.and_then(|before| {
                ordered[i..ordered.len().min(run_end + 1)]
                    .iter()
                    .map(|t| t.speaker)
                    .find(|&s| s != before)
            });
            events.push(Interruption { by });
            for t in &ordered[i..run_end] {
                note_end(&mut last_end, t);
            }
            i = run_end;
            continue;
        }
        let t = ordered[i];
        let cuts_in = last_end
            .iter()
            .any(|(&speaker, &end)| speaker != t.speaker && t.start < end - TIME_EPS);
        if cuts_in {
            events.push(Interruption {
                by: Some(t.speaker),
            });
        }
        note_end(&mut last_end, t);
        i += 1;
    }
    events
}

fn is_overlap(turn: &Turn) -> bool {
    turn.flags.iter().any(|f| f == OVERLAP_FLAG)
}

fn note_end(last_end: &mut HashMap<u32, f64>, turn: &Turn) {
    let end = last_end.entry(turn.speaker).or_insert(turn.end);
    *end = end.max(turn.end);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    fn events(turns: &[(u32, f64, f64, &str)], overlap: &[usize]) -> Vec<Option<u32>> {
        let mut t = transcript(turns);
        for &i in overlap {
            t.turns[i].flags.push(OVERLAP_FLAG.into());
        }
        interruptions(&t.turns).into_iter().map(|e| e.by).collect()
    }

    #[test]
    fn alternating_turns_without_overlap_have_no_interruptions() {
        let e = events(
            &[(1, 0.0, 2.0, "а"), (2, 2.5, 4.0, "б"), (1, 4.0, 5.0, "в")],
            &[],
        );
        assert!(e.is_empty());
    }

    #[test]
    fn turn_starting_before_other_speaker_ends_is_interruption_by_its_speaker() {
        let e = events(&[(1, 0.0, 5.0, "а"), (2, 4.0, 6.0, "б")], &[]);
        assert_eq!(e, vec![Some(2)]);
    }

    #[test]
    fn same_speaker_overlapping_own_turn_is_not_interruption() {
        let e = events(&[(1, 0.0, 5.0, "а"), (1, 4.0, 6.0, "б")], &[]);
        assert!(e.is_empty());
    }

    #[test]
    fn overlap_run_counts_once_and_blames_speaker_who_takes_over() {
        let e = events(
            &[
                (1, 0.0, 5.0, "а"),
                (1, 5.0, 6.0, "б"),
                (2, 6.0, 7.0, "в"),
                (2, 7.0, 9.0, "г"),
            ],
            &[1, 2],
        );
        assert_eq!(e, vec![Some(2)]);
    }

    #[test]
    fn short_cut_in_inside_overlap_blames_the_one_who_cut_in() {
        let e = events(
            &[(1, 0.0, 5.0, "а"), (2, 5.0, 6.0, "б"), (1, 6.0, 8.0, "в")],
            &[1],
        );
        assert_eq!(e, vec![Some(2)]);
    }

    #[test]
    fn overlap_with_one_speaker_around_has_unknown_author() {
        let e = events(
            &[(1, 0.0, 5.0, "а"), (1, 5.0, 6.0, "б"), (1, 6.0, 8.0, "в")],
            &[1],
        );
        assert_eq!(e, vec![None]);
    }

    #[test]
    fn overlap_at_the_very_start_has_unknown_author() {
        let e = events(&[(1, 0.0, 1.0, "а"), (2, 1.0, 3.0, "б")], &[0]);
        assert_eq!(e, vec![None]);
    }

    #[test]
    fn turns_are_taken_in_time_order() {
        let e = events(&[(2, 4.0, 6.0, "б"), (1, 0.0, 5.0, "а")], &[]);
        assert_eq!(e, vec![Some(2)]);
    }
}
