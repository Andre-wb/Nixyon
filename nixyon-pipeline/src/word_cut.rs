use nixyon_core::{Turn, Word};

use crate::WindowLabel;

/// Кусок речи становится одним или несколькими ходами. Если окна внутри куска
/// принадлежат разным говорящим, кусок режется по границе между словами, ближайшей
/// к точке смены (середина между центрами соседних окон разных говорящих).
/// Без времени слов или без смены говорящего кусок остаётся целым. Смена, для которой
/// не нашлось границы с хотя бы одним словом по обе стороны, пропускается.
pub(crate) fn cut_by_words(
    start: f64,
    end: f64,
    text: String,
    words: Option<Vec<Word>>,
    speaker: u32,
    windows: &[WindowLabel],
) -> Vec<Turn> {
    let whole =
        |text: String, words: Option<Vec<Word>>| vec![turn(speaker, start, end, text, words)];
    let changes = change_points(windows);
    let words = match words {
        Some(words) if words.len() > 1 && !changes.is_empty() => words,
        other => return whole(text, other),
    };

    // Граница j лежит между словами j и j + 1.
    let bounds: Vec<f64> = words
        .windows(2)
        .map(|w| (w[0].end + w[1].start) / 2.0)
        .collect();
    let mut cuts: Vec<(usize, u32)> = Vec::new();
    for (time, next_speaker) in changes {
        let first_free = cuts.last().map_or(0, |&(j, _)| j + 1);
        let best = (first_free..bounds.len()).min_by(|&a, &b| {
            (bounds[a] - time)
                .abs()
                .total_cmp(&(bounds[b] - time).abs())
        });
        if let Some(j) = best {
            cuts.push((j, next_speaker));
        }
    }
    if cuts.is_empty() {
        return whole(text, Some(words));
    }

    let first_speaker = windows.first().map_or(speaker, |w| w.speaker);
    let mut pieces: Vec<Turn> = Vec::new();
    let mut from = 0;
    let mut piece_start = start;
    let mut piece_speaker = first_speaker;
    for (j, next_speaker) in cuts {
        pieces.push(piece(
            piece_speaker,
            piece_start,
            bounds[j],
            &words[from..=j],
        ));
        from = j + 1;
        piece_start = bounds[j];
        piece_speaker = next_speaker;
    }
    pieces.push(piece(piece_speaker, piece_start, end, &words[from..]));
    merge_same_speaker(pieces)
}

// Время смены и новый говорящий: середина между центрами соседних окон.
fn change_points(windows: &[WindowLabel]) -> Vec<(f64, u32)> {
    let center = |w: &WindowLabel| (w.start + w.end) / 2.0;
    windows
        .windows(2)
        .filter(|w| w[0].speaker != w[1].speaker)
        .map(|w| ((center(&w[0]) + center(&w[1])) / 2.0, w[1].speaker))
        .collect()
}

pub(crate) fn piece(speaker: u32, start: f64, end: f64, words: &[Word]) -> Turn {
    let text = words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    turn(speaker, start, end, text, Some(words.to_vec()))
}

pub(crate) fn turn(
    speaker: u32,
    start: f64,
    end: f64,
    text: String,
    words: Option<Vec<Word>>,
) -> Turn {
    Turn {
        id: 0,
        speaker,
        start,
        end,
        text,
        words,
        flags: Vec::new(),
    }
}

// Пропущенная смена может оставить рядом две части одного говорящего.
fn merge_same_speaker(pieces: Vec<Turn>) -> Vec<Turn> {
    let mut merged: Vec<Turn> = Vec::with_capacity(pieces.len());
    for p in pieces {
        match merged.last_mut() {
            Some(last) if last.speaker == p.speaker => {
                last.end = p.end;
                last.text = format!("{} {}", last.text, p.text);
                if let (Some(a), Some(b)) = (last.words.as_mut(), p.words) {
                    a.extend(b);
                }
            }
            _ => merged.push(p),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start: f64, end: f64) -> Word {
        Word {
            text: text.into(),
            start,
            end,
        }
    }

    fn label(start: f64, end: f64, speaker: u32) -> WindowLabel {
        WindowLabel {
            start,
            end,
            speaker,
        }
    }

    fn words() -> Vec<Word> {
        vec![
            w("добрый", 0.0, 0.5),
            w("день", 0.6, 1.0),
            w("коллеги", 1.1, 1.8),
            w("привет", 2.2, 2.7),
            w("всем", 2.8, 3.2),
        ]
    }

    fn shape(turns: &[Turn]) -> Vec<(u32, &str, f64, f64)> {
        turns
            .iter()
            .map(|t| (t.speaker, t.text.as_str(), t.start, t.end))
            .collect()
    }

    #[test]
    fn change_inside_segment_cuts_at_nearest_word_boundary() {
        // Центры окон 0,75, 1,25, 1,75 (говорящий 1) и 2,25, 2,75 (говорящий 2):
        // смена в 2,0, ближайшая граница между «коллеги» и «привет» в 2,0.
        let windows = [
            label(0.0, 1.5, 1),
            label(0.5, 2.0, 1),
            label(1.0, 2.5, 1),
            label(1.5, 3.0, 2),
            label(2.0, 3.5, 2),
        ];
        let t = cut_by_words(0.0, 3.5, "x".into(), Some(words()), 1, &windows);
        assert_eq!(
            shape(&t),
            [
                (1, "добрый день коллеги", 0.0, 2.0),
                (2, "привет всем", 2.0, 3.5)
            ]
        );
        assert_eq!(t[1].words.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn change_point_lies_midway_between_window_centers() {
        // Центры окон 1,0 и 1,5: смена в 1,25, совпадает с границей между «а» и «б».
        let three = vec![w("а", 0.0, 1.2), w("б", 1.3, 1.5), w("в", 1.6, 3.0)];
        let windows = [label(0.25, 1.75, 1), label(0.75, 2.25, 2)];
        let t = cut_by_words(0.0, 3.0, "x".into(), Some(three), 1, &windows);
        assert_eq!(shape(&t), [(1, "а", 0.0, 1.25), (2, "б в", 1.25, 3.0)]);
    }

    #[test]
    fn single_speaker_keeps_the_segment_and_its_original_text() {
        let windows = [label(0.0, 1.5, 2), label(0.5, 2.0, 2)];
        let t = cut_by_words(
            0.0,
            3.5,
            "Добрый день, коллеги!".into(),
            Some(words()),
            2,
            &windows,
        );
        assert_eq!(shape(&t), [(2, "Добрый день, коллеги!", 0.0, 3.5)]);
    }

    #[test]
    fn without_word_times_nothing_is_cut() {
        let windows = [label(0.0, 1.5, 1), label(1.5, 3.0, 2)];
        let t = cut_by_words(0.0, 3.0, "а б".into(), None, 1, &windows);
        assert_eq!(shape(&t), [(1, "а б", 0.0, 3.0)]);
    }

    #[test]
    fn two_changes_give_three_pieces_and_a_back_and_forth_is_kept() {
        let windows = [
            label(0.0, 1.5, 1),
            label(0.5, 2.0, 2),
            label(1.0, 2.5, 2),
            label(1.5, 3.0, 2),
            label(2.0, 3.5, 1),
        ];
        // Смены в 1,0 и 2,5: ближайшие границы 1,05 и 2,75.
        let t = cut_by_words(0.0, 3.5, "x".into(), Some(words()), 2, &windows);
        assert_eq!(
            shape(&t),
            [
                (1, "добрый день", 0.0, 1.05),
                (2, "коллеги привет", 1.05, 2.75),
                (1, "всем", 2.75, 3.5)
            ]
        );
    }

    #[test]
    fn change_with_no_room_for_a_word_is_skipped() {
        let two = vec![w("да", 0.0, 0.4), w("нет", 0.6, 1.0)];
        let windows = [
            label(0.0, 0.5, 1),
            label(0.2, 0.7, 2),
            label(0.4, 0.9, 1),
            label(0.5, 1.0, 2),
        ];
        // Три смены, а граница между словами одна: остаются две части.
        let t = cut_by_words(0.0, 1.0, "x".into(), Some(two), 1, &windows);
        assert_eq!(shape(&t), [(1, "да", 0.0, 0.5), (2, "нет", 0.5, 1.0)]);
    }
}
