/// Границы кусков речи в сэмплах `[начало, конец)` с запасом `pad` с каждой стороны.
/// VAD режет кусок вплотную к речи и часто отрезает начало первого слова или конец
/// последнего, а модель на таком куске теряет слово целиком. Запас не заходит за
/// середину промежутка до соседнего куска, чтобы одни и те же слова не распознавались
/// дважды, и за края записи. Куски идут по порядку и не перекрываются.
pub(crate) fn padded_bounds(
    bounds: &[(usize, usize)],
    total: usize,
    pad: usize,
) -> Vec<(usize, usize)> {
    bounds
        .iter()
        .enumerate()
        .map(|(i, &(start, end))| {
            let left_limit = match i.checked_sub(1).and_then(|p| bounds.get(p)) {
                Some(&(_, prev_end)) => midpoint(prev_end, start),
                None => 0,
            };
            let right_limit = match bounds.get(i + 1) {
                Some(&(next_start, _)) => midpoint(end, next_start),
                None => total,
            };
            let from = start.saturating_sub(pad).max(left_limit).min(start);
            let to = end.saturating_add(pad).min(right_limit).max(end).min(total);
            (from, to.max(from))
        })
        .collect()
}

// Середина промежутка; если куски соприкасаются или перекрываются, граница остаётся
// там, где её поставил VAD.
fn midpoint(left_end: usize, right_start: usize) -> usize {
    if right_start <= left_end {
        right_start
    } else {
        left_end + (right_start - left_end) / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lone_region_gets_the_full_pad_inside_the_recording() {
        assert_eq!(
            padded_bounds(&[(1_000, 2_000)], 10_000, 300),
            [(700, 2_300)]
        );
    }

    #[test]
    fn pad_stops_at_the_edges_of_the_recording() {
        assert_eq!(padded_bounds(&[(100, 9_900)], 10_000, 300), [(0, 10_000)]);
    }

    #[test]
    fn neighbours_split_a_short_gap_in_the_middle_and_never_overlap() {
        let padded = padded_bounds(&[(1_000, 2_000), (2_200, 3_000)], 10_000, 300);
        assert_eq!(padded, [(700, 2_100), (2_100, 3_300)]);
    }

    #[test]
    fn a_wide_gap_leaves_the_full_pad_on_both_sides() {
        let padded = padded_bounds(&[(1_000, 2_000), (5_000, 6_000)], 10_000, 300);
        assert_eq!(padded, [(700, 2_300), (4_700, 6_300)]);
    }

    #[test]
    fn touching_or_overlapping_regions_keep_their_vad_edges_between_them() {
        let padded = padded_bounds(&[(1_000, 2_000), (1_900, 3_000)], 10_000, 300);
        assert_eq!(padded, [(700, 2_000), (1_900, 3_300)]);
    }

    #[test]
    fn no_regions_give_no_bounds() {
        assert!(padded_bounds(&[], 10_000, 300).is_empty());
    }
}
