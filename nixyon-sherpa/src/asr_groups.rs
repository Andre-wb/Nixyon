/// Группы подряд идущих кусков речи `[начало, конец)` в сэмплах для распознавания одним
/// отрезком звука. Кусок, распознанный отдельно, кончается для модели концом записи:
/// GigaAM с пунктуацией ставит в конце точку, и фраза, разрезанная паузой, рвётся на
/// предложения. В группе пауза между соседними кусками не длиннее `max_gap`, а вся группа,
/// от начала первого куска до конца последнего, не длиннее `max_len`. Кусок длиннее
/// `max_len` идёт один. Так же группирует приложение на Android (AsrGroups.kt).
pub(crate) fn asr_groups(
    bounds: &[(usize, usize)],
    max_len: usize,
    max_gap: usize,
) -> Vec<Vec<(usize, usize)>> {
    let mut groups: Vec<Vec<(usize, usize)>> = Vec::new();
    for &piece in bounds {
        let fits = groups.last().is_some_and(|group| {
            let (first, last) = (group[0], group[group.len() - 1]);
            piece.0.saturating_sub(last.1) <= max_gap && piece.1 - first.0 <= max_len
        });
        match groups.last_mut() {
            Some(group) if fits => group.push(piece),
            _ => groups.push(vec![piece]),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_with_short_pauses_share_a_group() {
        let bounds = [(0, 1_000), (1_500, 3_000), (3_200, 4_000)];
        assert_eq!(asr_groups(&bounds, 10_000, 1_000), [bounds.to_vec()]);
    }

    #[test]
    fn a_long_pause_starts_a_new_group() {
        let bounds = [(0, 1_000), (2_500, 3_000)];
        assert_eq!(
            asr_groups(&bounds, 10_000, 1_000),
            [vec![(0, 1_000)], vec![(2_500, 3_000)]]
        );
    }

    #[test]
    fn a_group_never_grows_past_max_len() {
        let bounds = [(0, 4_000), (4_100, 8_000), (8_100, 12_000)];
        assert_eq!(
            asr_groups(&bounds, 10_000, 1_000),
            [vec![(0, 4_000), (4_100, 8_000)], vec![(8_100, 12_000)]]
        );
    }

    #[test]
    fn a_piece_longer_than_max_len_stays_alone() {
        let bounds = [(0, 500), (600, 12_000), (12_100, 12_500)];
        assert_eq!(
            asr_groups(&bounds, 10_000, 1_000),
            [vec![(0, 500)], vec![(600, 12_000)], vec![(12_100, 12_500)]]
        );
    }

    #[test]
    fn no_pieces_give_no_groups() {
        assert!(asr_groups(&[], 10_000, 1_000).is_empty());
    }
}
