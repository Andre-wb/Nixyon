use nixyon_core::{FIRST_OVERLAP_CLASS, POWERSET_PAIRS, PowersetWindow, Turn, speaker_probability};

/// Локальных говорящих в окне pyannote.
const LOCAL_SPEAKERS: usize = 3;

/// Второй говорящий перекрытия `start..end` по локальным говорящим окон модели. В каждом
/// окне, которое видит перекрытие, локальный говорящий сопоставляется нашему: тому, чьи
/// ходы с текстом больше всего совпадают по времени с его активностью в окне. Кадры
/// перекрытия голосуют за пару из самого вероятного класса перекрытия весом этой
/// вероятности. Итог: говорящий не из `heard` с наибольшим числом голосов.
pub(crate) fn local_speaker(
    windows: &[PowersetWindow],
    turns: &[Turn],
    start: f64,
    end: f64,
    heard: &[u32],
) -> Option<u32> {
    let spoken: Vec<&Turn> = turns.iter().filter(|t| !t.text.trim().is_empty()).collect();
    let mut votes: Vec<(u32, f32)> = Vec::new();
    for w in windows {
        let inside: Vec<usize> = (0..w.frames.len())
            .filter(|&i| (start..end).contains(&w.frame_time(i)))
            .collect();
        if inside.is_empty() {
            continue;
        }
        let local = local_to_ours(w, &spoken);
        for i in inside {
            let frame = &w.frames[i];
            let Some((pair, weight)) = frame[FIRST_OVERLAP_CLASS..]
                .iter()
                .copied()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(&b.1))
            else {
                continue;
            };
            let (a, b) = POWERSET_PAIRS[pair];
            for speaker in [local[a], local[b]].into_iter().flatten() {
                match votes.iter_mut().find(|(s, _)| *s == speaker) {
                    Some((_, v)) => *v += weight,
                    None => votes.push((speaker, weight)),
                }
            }
        }
    }
    votes
        .into_iter()
        .filter(|(s, v)| !heard.contains(s) && *v > 0.0)
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(s, _)| s)
}

// Наш говорящий каждого локального говорящего окна; None, если его активность ни с чьими
// ходами не совпала.
fn local_to_ours(w: &PowersetWindow, spoken: &[&Turn]) -> [Option<u32>; LOCAL_SPEAKERS] {
    let from = w.frame_time(0);
    let to = w.frame_time(w.frames.len().saturating_sub(1));
    let near: Vec<&Turn> = spoken
        .iter()
        .copied()
        .filter(|t| t.start <= to && from <= t.end)
        .collect();
    let mut result = [None; LOCAL_SPEAKERS];
    for (k, slot) in result.iter_mut().enumerate() {
        let mut scores: Vec<(u32, f32)> = Vec::new();
        for (i, frame) in w.frames.iter().enumerate() {
            let time = w.frame_time(i);
            let p = speaker_probability(frame, k);
            for t in near.iter().filter(|t| t.start <= time && time < t.end) {
                match scores.iter_mut().find(|(s, _)| *s == t.speaker) {
                    Some((_, v)) => *v += p,
                    None => scores.push((t.speaker, p)),
                }
            }
        }
        *slot = scores
            .into_iter()
            .filter(|(_, v)| *v > 0.0)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .map(|(s, _)| s);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    // Кадры по 1 с: локальный 0 говорит один, затем локальные 0 и 1 вместе, затем 1 один.
    const ZERO: [f32; 7] = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    const BOTH: [f32; 7] = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    const ONE: [f32; 7] = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];

    fn window() -> PowersetWindow {
        PowersetWindow {
            first_frame_sec: 0.5,
            frame_sec: 1.0,
            frames: vec![ZERO, ZERO, ZERO, BOTH, ONE, ONE, ONE],
        }
    }

    #[test]
    fn the_other_local_voice_of_the_overlap_is_matched_to_its_turns() {
        // Говорящий 7 звучит 0..4, говорящий 9 с 4 с; перекрытие 3..4 слышно только у 7.
        let t = transcript(&[(7, 0.0, 4.0, "раз"), (9, 4.0, 7.0, "два")]);
        assert_eq!(
            local_speaker(&[window()], &t.turns, 3.0, 4.0, &[7]),
            Some(9)
        );
    }

    #[test]
    fn no_second_voice_when_both_are_heard() {
        let t = transcript(&[(7, 0.0, 4.0, "раз"), (9, 4.0, 7.0, "два")]);
        assert_eq!(
            local_speaker(&[window()], &t.turns, 3.0, 4.0, &[7, 9]),
            None
        );
    }

    #[test]
    fn local_voice_without_matching_turns_gives_nothing() {
        let t = transcript(&[(7, 0.0, 4.0, "раз")]);
        assert_eq!(local_speaker(&[window()], &t.turns, 3.0, 4.0, &[7]), None);
    }
}
