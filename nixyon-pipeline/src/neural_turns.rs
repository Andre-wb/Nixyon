use nixyon_core::{OVERLAP_FLAG, SegmentResult, Turn, VoiceSpan, Word};

use crate::neural_labels::neural_labels;
use crate::sample_names::SpeakerCenter;
use crate::speaker_labels::centers_by_speaker;
use crate::word_cut::{piece, turn};
use crate::word_speakers::{LabeledSpan, WordSpeaker, word_speakers};
use crate::words::tokens_to_words;
use crate::{PipelineOptions, SpeakerCount};

/// Ходы по нейросегментации. Текст остаётся от кусков VAD, говорящего каждое слово берёт
/// по середине слова из отрезков нейросегментации. Кусок режется там, где меняется
/// говорящий или начинается и кончается перекрытие; часть с перекрытием помечается.
/// Номера спикеров с 1 в порядке первого появления. Куски идут по времени.
/// Вместе с ходами отдаёт центры голосов спикеров.
pub(crate) fn neural_turns(
    segments: &[SegmentResult],
    spans: &[VoiceSpan],
    count: SpeakerCount,
    options: &PipelineOptions,
) -> (Vec<Turn>, Vec<SpeakerCenter>) {
    let labels = neural_labels(segments, spans, count, options);
    let labeled: Vec<LabeledSpan> = spans
        .iter()
        .zip(&labels.spans)
        .filter_map(|(s, l)| l.map(|l| (s.start, s.end, l)))
        .collect();

    let mut turns: Vec<Turn> = Vec::new();
    for (s, label) in segments.iter().zip(&labels.segments) {
        let fallback = label.unwrap_or(0);
        let text = s.text.trim().to_string();
        let words = tokens_to_words(&s.tokens, &s.token_times, s.start, s.end);
        match words {
            Some(words) if !words.is_empty() => {
                let inside: Vec<LabeledSpan> = labeled
                    .iter()
                    .copied()
                    .filter(|&(a, b, _)| a < s.end && s.start < b)
                    .collect();
                let speakers = word_speakers(&words, &inside, fallback);
                turns.extend(split(s.start, s.end, text, &words, &speakers));
            }
            other => turns.push(turn(cluster_id(fallback), s.start, s.end, text, other)),
        }
    }
    let order = renumber(&mut turns);
    let centers = centers_by_speaker(&order, &labels.centers);
    (turns, centers)
}

// Подряд идущие слова с одним говорящим и одним признаком перекрытия дают часть.
// Одна часть: кусок целиком с исходным текстом.
fn split(
    start: f64,
    end: f64,
    text: String,
    words: &[Word],
    speakers: &[WordSpeaker],
) -> Vec<Turn> {
    let mut groups: Vec<(usize, usize, WordSpeaker)> = Vec::new();
    for (i, &ws) in speakers.iter().enumerate() {
        match groups.last_mut() {
            Some((_, to, last)) if *last == ws => *to = i,
            _ => groups.push((i, i, ws)),
        }
    }
    let flagged = |mut t: Turn, ws: WordSpeaker| {
        if ws.overlap {
            t.flags.push(OVERLAP_FLAG.into());
        }
        t
    };
    if let [(_, _, ws)] = groups.as_slice() {
        return vec![flagged(
            turn(
                cluster_id(ws.speaker),
                start,
                end,
                text,
                Some(words.to_vec()),
            ),
            *ws,
        )];
    }
    let last = groups.len().saturating_sub(1);
    groups
        .iter()
        .enumerate()
        .map(|(g, &(from, to, ws))| {
            // Граница частей посередине между соседними словами.
            let a = if g == 0 {
                start
            } else {
                (words[from - 1].end + words[from].start) / 2.0
            };
            let b = if g == last {
                end
            } else {
                (words[to].end + words[to + 1].start) / 2.0
            };
            flagged(piece(cluster_id(ws.speaker), a, b, &words[from..=to]), ws)
        })
        .collect()
}

fn cluster_id(label: usize) -> u32 {
    u32::try_from(label).unwrap_or(u32::MAX)
}

// Номера кластеров меняются на номера спикеров; итог: кластер каждого спикера по порядку.
fn renumber(turns: &mut [Turn]) -> Vec<usize> {
    let mut order: Vec<u32> = Vec::new();
    for t in turns.iter_mut() {
        let index = order
            .iter()
            .position(|&o| o == t.speaker)
            .unwrap_or_else(|| {
                order.push(t.speaker);
                order.len() - 1
            });
        t.speaker = u32::try_from(index + 1).unwrap_or(u32::MAX);
    }
    order
        .into_iter()
        .map(|c| usize::try_from(c).unwrap_or(usize::MAX))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{segment, voice};

    fn span(start: f64, end: f64, embedding: Vec<f32>) -> VoiceSpan {
        VoiceSpan {
            start,
            end,
            embedding,
        }
    }

    // Токены по словам с маркером начала, время каждого токена задано.
    fn spoken(start: f64, end: f64, words: &[(&str, f32)], embedding: Vec<f32>) -> SegmentResult {
        let text: Vec<&str> = words.iter().map(|w| w.0).collect();
        let mut s = segment(start, end, &text.join(" "), embedding);
        s.tokens = words.iter().map(|w| format!("▁{}", w.0)).collect();
        s.token_times = words.iter().map(|w| w.1).collect();
        s
    }

    fn shape(turns: &[Turn]) -> Vec<(u32, &str, bool)> {
        turns
            .iter()
            .map(|t| {
                (
                    t.speaker,
                    t.text.as_str(),
                    t.flags.iter().any(|f| f == OVERLAP_FLAG),
                )
            })
            .collect()
    }

    #[test]
    fn segment_is_split_where_another_voice_joins_and_the_overlap_is_marked() {
        // Кусок 0..6: голос 1 до 4 с, голос 2 с 3 с; перекрытие 3..4 приходится на «в».
        let seg = spoken(
            0.0,
            6.0,
            &[("а", 0.5), ("б", 2.0), ("в", 3.2), ("г", 4.5)],
            voice(1, 0.0),
        );
        let spans = vec![
            span(0.0, 4.0, voice(1, 0.0)),
            span(3.0, 6.0, voice(2, 0.0)),
            span(7.0, 9.0, voice(2, 0.1)),
        ];
        let (t, _) = neural_turns(
            &[seg],
            &spans,
            SpeakerCount::Known(2),
            &PipelineOptions::default(),
        );
        assert_eq!(
            shape(&t),
            [(1, "а б", false), (1, "в", true), (2, "г", false)]
        );
        // Границы частей посередине паузы между словами: слово кончается через кадр
        // после своего токена, «б» в 2,04 с, «в» в 3,24 с.
        let bounds: Vec<(f64, f64)> = t.iter().map(|x| (x.start, x.end)).collect();
        for (got, want) in bounds.iter().zip([(0.0, 2.62), (2.62, 3.87), (3.87, 6.0)]) {
            assert!(
                (got.0 - want.0).abs() < 1e-6 && (got.1 - want.1).abs() < 1e-6,
                "{bounds:?}"
            );
        }
    }

    #[test]
    fn one_voice_keeps_the_segment_whole_with_its_original_text() {
        let mut seg = spoken(0.0, 3.0, &[("Добрый", 0.2), ("день!", 1.0)], voice(1, 0.0));
        seg.text = "Добрый день!".into();
        let spans = vec![span(0.0, 3.0, voice(1, 0.0))];
        let (t, _) = neural_turns(
            &[seg],
            &spans,
            SpeakerCount::Auto,
            &PipelineOptions::default(),
        );
        assert_eq!(shape(&t), [(1, "Добрый день!", false)]);
        assert_eq!((t[0].start, t[0].end), (0.0, 3.0));
    }

    #[test]
    fn segment_without_word_times_is_one_turn_of_its_segment_speaker() {
        let segs = vec![
            segment(0.0, 3.0, "раз", voice(1, 0.0)),
            segment(4.0, 7.0, "два", voice(2, 0.0)),
        ];
        let spans = vec![span(0.0, 3.0, voice(1, 0.0)), span(4.0, 7.0, voice(2, 0.0))];
        let (t, _) = neural_turns(
            &segs,
            &spans,
            SpeakerCount::Known(2),
            &PipelineOptions::default(),
        );
        assert_eq!(shape(&t), [(1, "раз", false), (2, "два", false)]);
    }

    #[test]
    fn speakers_are_numbered_by_first_appearance() {
        let segs = vec![
            spoken(0.0, 3.0, &[("а", 0.5)], voice(5, 0.0)),
            spoken(4.0, 7.0, &[("б", 0.5)], voice(3, 0.0)),
            spoken(8.0, 11.0, &[("в", 0.5)], voice(5, 0.1)),
        ];
        let spans: Vec<VoiceSpan> = segs
            .iter()
            .map(|s| span(s.start, s.end, s.embedding.clone()))
            .collect();
        let (t, _) = neural_turns(
            &segs,
            &spans,
            SpeakerCount::Known(2),
            &PipelineOptions::default(),
        );
        let ids: Vec<u32> = t.iter().map(|x| x.speaker).collect();
        assert_eq!(ids, [1, 2, 1]);
    }
}
