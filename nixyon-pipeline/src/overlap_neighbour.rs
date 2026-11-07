use nixyon_core::Turn;

/// Ход другого говорящего дальше этого от перекрытия не считается его участником.
const MAX_NEIGHBOUR_GAP_SEC: f64 = 1.0;

/// Второй говорящий перекрытия `start..end` по соседним ходам: из говорящих, которых нет
/// в `heard`, тот, чей ход с текстом ближе всех к перекрытию, не дальше
/// `MAX_NEIGHBOUR_GAP_SEC`. Обычно это тот, кто перебил, или тот, кого перебили.
pub(crate) fn neighbour_speaker(
    turns: &[Turn],
    start: f64,
    end: f64,
    heard: &[u32],
) -> Option<u32> {
    turns
        .iter()
        .filter(|t| !heard.contains(&t.speaker) && !t.text.trim().is_empty())
        .map(|t| (t.speaker, (t.start - end).max(start - t.end).max(0.0)))
        .filter(|&(_, gap)| gap <= MAX_NEIGHBOUR_GAP_SEC)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(speaker, _)| speaker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    #[test]
    fn the_nearest_other_speaker_is_the_second_voice() {
        let t = transcript(&[
            (1, 0.0, 5.0, "раз"),
            (2, 5.3, 8.0, "два"),
            (3, 9.0, 12.0, "три"),
        ]);
        assert_eq!(neighbour_speaker(&t.turns, 4.5, 5.0, &[1]), Some(2));
    }

    #[test]
    fn speakers_already_heard_are_skipped() {
        let t = transcript(&[
            (1, 0.0, 5.0, "раз"),
            (2, 5.3, 8.0, "два"),
            (3, 5.5, 6.0, "три"),
        ]);
        assert_eq!(neighbour_speaker(&t.turns, 4.5, 5.0, &[1, 2]), Some(3));
    }

    #[test]
    fn far_turns_give_no_second_voice() {
        let t = transcript(&[(1, 0.0, 5.0, "раз"), (2, 6.5, 8.0, "два")]);
        assert_eq!(neighbour_speaker(&t.turns, 4.5, 5.0, &[1]), None);
    }

    #[test]
    fn turns_without_text_are_not_neighbours() {
        let t = transcript(&[(1, 0.0, 5.0, "раз"), (2, 5.0, 5.5, "")]);
        assert_eq!(neighbour_speaker(&t.turns, 4.5, 5.0, &[1]), None);
    }
}
