use crate::IndexClip;

// Зерно постоянное: при том же объёме выбираются те же клипы, и прогоны сравнимы.
const SEED: u64 = 0x6e69_7879_6f6e_2121;

/// Клипы набора в перемешанном, но всегда одинаковом порядке, пока их длительность не
/// наберёт `seconds`. `None`: весь набор. Перемешивание нужно, чтобы выборка не зависела
/// от порядка в индексе (там клипы часто идут по дикторам или по эпизодам).
pub fn pick_clips(clips: &[IndexClip], seconds: Option<f64>) -> Vec<&IndexClip> {
    let mut order: Vec<&IndexClip> = clips.iter().collect();
    let mut state = SEED;
    for i in (1..order.len()).rev() {
        state = split_mix(state);
        // Остаток от деления меньше i + 1, значит помещается в usize.
        let j = (state % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }
    let Some(budget) = seconds else {
        return order;
    };
    let mut total = 0.0;
    order
        .into_iter()
        .take_while(|clip| {
            let take = total < budget;
            total += clip.duration;
            take
        })
        .collect()
}

fn split_mix(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clips(n: usize) -> Vec<IndexClip> {
        (0..n)
            .map(|i| IndexClip {
                id: format!("c{i:03}"),
                audio: format!("a/{i}.wav"),
                text: None,
                duration: 10.0,
                speakers: Some(1),
                truth: None,
            })
            .collect()
    }

    #[test]
    fn the_whole_set_is_taken_without_a_budget() {
        let all = clips(50);
        assert_eq!(pick_clips(&all, None).len(), 50);
    }

    #[test]
    fn clips_are_taken_until_the_budget_is_reached() {
        let all = clips(50);
        assert_eq!(pick_clips(&all, Some(35.0)).len(), 4);
        assert_eq!(pick_clips(&all, Some(30.0)).len(), 3);
        assert_eq!(pick_clips(&all, Some(10_000.0)).len(), 50);
    }

    #[test]
    fn the_same_budget_gives_the_same_clips_and_they_are_not_the_first_ones() {
        let all = clips(50);
        let first: Vec<&str> = pick_clips(&all, Some(50.0))
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        let again: Vec<&str> = pick_clips(&all, Some(50.0))
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(first, again);
        assert_ne!(first, ["c000", "c001", "c002", "c003", "c004"]);
    }
}
