/// Сколько отрезков записи слушает определение языка.
pub const LANGUAGE_CLIPS: usize = 5;
/// Отрезок не длиннее 30 с при 16 кГц: дальше Whisper не слушает.
pub const MAX_LANGUAGE_CLIP_SAMPLES: usize = 480_000;

/// Отрезки для определения языка: запись делится на `LANGUAGE_CLIPS` равных частей по
/// времени, из каждой берётся самый длинный кусок речи (кусок относится к части по своей
/// середине), обрезанный до `MAX_LANGUAGE_CLIP_SAMPLES`. Так один разговорчивый участник
/// в начале записи не решает за всю встречу. Часть без речи отрезка не даёт.
/// `bounds`: куски речи в сэмплах, `total`: длина записи в сэмплах.
pub fn language_clips(bounds: &[(usize, usize)], total: usize) -> Vec<(usize, usize)> {
    if total == 0 {
        return Vec::new();
    }
    let mut best: [Option<(usize, usize)>; LANGUAGE_CLIPS] = [None; LANGUAGE_CLIPS];
    for &(from, to) in bounds.iter().filter(|(from, to)| to > from) {
        let middle = from + (to - from) / 2;
        let part = (middle.min(total - 1) * LANGUAGE_CLIPS / total).min(LANGUAGE_CLIPS - 1);
        if best[part].is_none_or(|(a, b)| to - from > b - a) {
            best[part] = Some((from, to));
        }
    }
    best.into_iter()
        .flatten()
        .map(|(from, to)| (from, to.min(from + MAX_LANGUAGE_CLIP_SAMPLES)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_longest_piece_of_each_fifth() {
        // Запись 100 сэмплов: части по 20.
        let bounds = [(0, 5), (6, 18), (25, 30), (41, 59), (90, 99)];
        assert_eq!(
            language_clips(&bounds, 100),
            vec![(6, 18), (25, 30), (41, 59), (90, 99)]
        );
    }

    #[test]
    fn piece_belongs_to_the_part_of_its_middle() {
        // Середина куска (15, 35) на 25: вторая часть.
        assert_eq!(language_clips(&[(15, 35)], 100), vec![(15, 35)]);
        assert_eq!(language_clips(&[(15, 35), (21, 39)], 100), vec![(15, 35)]);
    }

    #[test]
    fn long_piece_is_cut_to_whisper_window() {
        let total = 10 * MAX_LANGUAGE_CLIP_SAMPLES;
        let clips = language_clips(&[(100, 100 + 2 * MAX_LANGUAGE_CLIP_SAMPLES)], total);
        assert_eq!(clips, vec![(100, 100 + MAX_LANGUAGE_CLIP_SAMPLES)]);
    }

    #[test]
    fn no_speech_gives_no_clips() {
        assert!(language_clips(&[], 100).is_empty());
        assert!(language_clips(&[(3, 3)], 100).is_empty());
        assert!(language_clips(&[(0, 5)], 0).is_empty());
    }

    #[test]
    fn piece_past_the_end_goes_to_the_last_part() {
        assert_eq!(language_clips(&[(98, 120)], 100), vec![(98, 120)]);
    }
}
