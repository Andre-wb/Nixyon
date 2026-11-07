use nixyon_cluster::{cosine, unit};
use nixyon_core::{SPEAKER_UNSURE_FLAG, SegmentResult, Turn, VoiceSpan};

/// Ход, голос которого похож на центр своего спикера меньше этого косинуса, помечается
/// неуверенным: спикер у него лучший из найденных, но может быть не тем. Замер на 16
/// записях с эталоном (5 минут встречи AMI, 13 отрывков встреч по 30 с, пример pyannote,
/// 3 голоса на другом языке) и 4 синтезированных диалогах: с CAM++ помечено 23 хода из 148, среди них 6 из 10 ошибочных; с
/// ReDimNet2-B6 9 из 129, среди них 3 из 6. Среди помеченных ошибочен каждый третий-
/// четвёртый ход, среди остальных 3 %, поэтому спикер остаётся, а не заменяется на
/// «не определён».
pub const SPEAKER_UNSURE_COSINE: f64 = 0.45;

/// Помечает ходы куска, где голос не похож уверенно на спикера хода. Спикер в ходах ещё
/// номер кластера. Сходство хода: косинус отрезков нейросегментации этого кластера внутри
/// хода к его центру, средний по времени; нет таких отрезков: косинус эмбеддинга куска.
/// Нет и его: спикер взят у соседа по времени, ход тоже неуверенный. Когда центров нет
/// совсем, сравнивать не с чем и ничего не помечается.
pub(crate) fn mark_speaker_unsure(
    turns: &mut [Turn],
    segment: &SegmentResult,
    spans: &[VoiceSpan],
    span_labels: &[Option<usize>],
    centers: &[Option<Vec<f32>>],
) {
    if centers.iter().all(Option::is_none) {
        return;
    }
    for t in turns {
        let Some(center) = usize::try_from(t.speaker)
            .ok()
            .and_then(|l| centers.get(l)?.as_ref())
        else {
            t.flags.push(SPEAKER_UNSURE_FLAG.into());
            continue;
        };
        let similarity = spans_similarity(t, spans, span_labels, center)
            .or_else(|| unit(&segment.embedding).map(|u| cosine(center, &u)));
        if similarity.is_none_or(|s| s < SPEAKER_UNSURE_COSINE) {
            t.flags.push(SPEAKER_UNSURE_FLAG.into());
        }
    }
}

// Косинус отрезков кластера хода к центру, средний по времени их пересечения с ходом.
fn spans_similarity(
    t: &Turn,
    spans: &[VoiceSpan],
    span_labels: &[Option<usize>],
    center: &[f32],
) -> Option<f64> {
    let mut time = 0.0;
    let mut sum = 0.0;
    for (s, label) in spans.iter().zip(span_labels) {
        let overlap = s.end.min(t.end) - s.start.max(t.start);
        if overlap <= 0.0 || label.and_then(|l| u32::try_from(l).ok()) != Some(t.speaker) {
            continue;
        }
        if let Some(u) = unit(&s.embedding) {
            time += overlap;
            sum += overlap * cosine(center, &u);
        }
    }
    (time > 0.0).then(|| sum / time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{segment, transcript, voice};

    fn span(start: f64, end: f64, embedding: Vec<f32>) -> VoiceSpan {
        VoiceSpan {
            start,
            end,
            embedding,
        }
    }

    fn unsure(turns: &[Turn]) -> Vec<bool> {
        turns
            .iter()
            .map(|t| t.flags.iter().any(|f| f == SPEAKER_UNSURE_FLAG))
            .collect()
    }

    // Голос 1 и голос с долей голоса 2: косинус к центру голоса 1 около 0,4.
    fn mixed() -> Vec<f32> {
        let a = voice(1, 0.0);
        let b = voice(2, 0.0);
        a.iter().zip(&b).map(|(x, y)| 0.4 * x + 0.9 * y).collect()
    }

    #[test]
    fn turn_whose_voice_is_far_from_its_speaker_is_marked() {
        let centers = vec![unit(&voice(1, 0.0)), unit(&voice(2, 0.0))];
        let spans = vec![
            span(0.0, 2.0, voice(1, 0.05)),
            span(2.0, 4.0, mixed()),
            span(4.0, 6.0, voice(2, 0.0)),
        ];
        let labels = vec![Some(0), Some(0), Some(1)];
        // Спикер ходов пока номер кластера.
        let mut turns =
            transcript(&[(0, 0.0, 2.0, "а"), (0, 2.0, 4.0, "б"), (0, 4.0, 6.0, "в")]).turns;
        let seg = segment(0.0, 6.0, "а б в", voice(1, 0.0));
        mark_speaker_unsure(&mut turns, &seg, &spans, &labels, &centers);
        // Третий ход кластера 0 без своих отрезков: сходство по эмбеддингу куска, он похож.
        assert_eq!(unsure(&turns), [false, true, false]);
    }

    #[test]
    fn without_spans_the_segment_voice_decides_and_without_any_voice_the_turn_is_a_guess() {
        let centers = vec![unit(&voice(1, 0.0)), unit(&voice(2, 0.0))];
        let mut near = transcript(&[(1, 0.0, 2.0, "да")]).turns;
        mark_speaker_unsure(
            &mut near,
            &segment(0.0, 2.0, "да", voice(2, 0.1)),
            &[],
            &[],
            &centers,
        );
        assert_eq!(unsure(&near), [false]);
        let mut far = transcript(&[(0, 0.0, 2.0, "да")]).turns;
        mark_speaker_unsure(
            &mut far,
            &segment(0.0, 2.0, "да", mixed()),
            &[],
            &[],
            &centers,
        );
        assert_eq!(unsure(&far), [true]);
        let mut guess = transcript(&[(1, 0.0, 2.0, "да")]).turns;
        mark_speaker_unsure(
            &mut guess,
            &segment(0.0, 2.0, "да", vec![]),
            &[],
            &[],
            &centers,
        );
        assert_eq!(unsure(&guess), [true]);
    }

    #[test]
    fn nothing_is_marked_when_there_are_no_voices_to_compare_with() {
        let mut turns = transcript(&[(0, 0.0, 2.0, "да")]).turns;
        mark_speaker_unsure(
            &mut turns,
            &segment(0.0, 2.0, "да", vec![]),
            &[],
            &[],
            &[None],
        );
        assert_eq!(unsure(&turns), [false]);
    }
}
