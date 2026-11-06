/// Куски `plain`, а вместо куска длиной от `long` сэмплов части кусков `clean` внутри
/// него. Обе последовательности по возрастанию и без перекрытий, итог тоже. Если внутри
/// длинного куска в `clean` речи нет, от него ничего не остаётся. Перенос
/// `SpeechFinder.splitLong` из Kotlin-плагина один к одному.
pub(crate) fn split_long(
    plain: &[(usize, usize)],
    clean: &[(usize, usize)],
    long: usize,
) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(plain.len());
    let mut first = 0;
    for &(a, b) in plain {
        if b - a < long {
            out.push((a, b));
            continue;
        }
        while first < clean.len() && clean[first].1 <= a {
            first += 1;
        }
        let mut i = first;
        while i < clean.len() && clean[i].0 < b {
            let from = a.max(clean[i].0);
            let to = b.min(clean[i].1);
            if to > from {
                out.push((from, to));
            }
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Те же случаи, что в LongPiecesTest.kt.
    const LONG: usize = 250;

    #[test]
    fn without_long_pieces_the_plain_pieces_stay_as_they_are() {
        let plain = [(0, 100), (150, 249), (300, 320)];
        assert_eq!(
            split_long(&plain, &[(10, 20), (160, 170), (400, 500)], LONG),
            plain
        );
        assert_eq!(split_long(&plain, &[], LONG), plain);
    }

    #[test]
    fn a_long_piece_is_replaced_by_the_clean_pieces_inside_it_cut_at_its_edges() {
        let plain = [(0, 50), (100, 400), (450, 500)];
        let clean = [(10, 40), (90, 150), (200, 250), (380, 460), (470, 480)];
        assert_eq!(
            split_long(&plain, &clean, LONG),
            [(0, 50), (100, 150), (200, 250), (380, 400), (450, 500)]
        );
    }

    #[test]
    fn a_long_piece_without_clean_speech_is_dropped() {
        assert_eq!(
            split_long(&[(0, 300), (500, 520)], &[(310, 400), (505, 510)], LONG),
            [(500, 520)]
        );
    }

    #[test]
    fn a_clean_piece_across_two_long_pieces_goes_to_both() {
        let plain = [(0, 250), (260, 600)];
        let clean = [(100, 120), (200, 300), (590, 700)];
        assert_eq!(
            split_long(&plain, &clean, LONG),
            [(100, 120), (200, 250), (260, 300), (590, 600)]
        );
    }
}
