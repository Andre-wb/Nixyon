use nixyon_core::{OVERLAP_FLAG, PowersetWindow, Turn};

use crate::model_overlaps::model_overlaps;
use crate::overlap_local_speakers::local_speaker;
use crate::overlap_neighbour::neighbour_speaker;
use crate::overlap_spans::overlap_spans;
use crate::word_cut::turn;
use crate::word_speakers::LabeledSpan;

/// Перекрытие короче этого не даёт хода без текста.
const MIN_SILENT_OVERLAP_SEC: f64 = 0.3;

/// Откуда берутся ходы без текста в перекрытиях. Распознаватель слышит в перекрытии один
/// голос, слова второго теряются; ход без текста показывает, что второй человек говорил.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilentOverlaps {
    /// Ходов без текста нет.
    Off,
    /// Перекрытие и оба говорящих по пересечению отрезков нейросегментации sherpa.
    Spans,
    /// Перекрытие по кадрам модели сегментации, второй говорящий по соседнему ходу.
    ModelNeighbour,
    /// Перекрытие по кадрам модели сегментации, второй говорящий по локальным говорящим
    /// окон модели.
    ModelLocalSpeakers,
}

/// Добавляет ходы без текста с пометкой перекрытия. Говорящий получает такой ход, только
/// если в этом месте у него нет других ходов. Ходы снова идут по времени. Номера
/// говорящих в `turns` и `labeled`: номера кластеров.
pub(crate) fn add_silent_overlaps(
    turns: &mut Vec<Turn>,
    labeled: &[LabeledSpan],
    windows: &[PowersetWindow],
    mode: SilentOverlaps,
    threshold: f32,
) {
    let found: Vec<(f64, f64, u32)> = match mode {
        SilentOverlaps::Off => Vec::new(),
        SilentOverlaps::Spans => overlap_spans(labeled, MIN_SILENT_OVERLAP_SEC)
            .into_iter()
            .map(|(start, end, label)| (start, end, cluster_id(label)))
            .collect(),
        SilentOverlaps::ModelNeighbour | SilentOverlaps::ModelLocalSpeakers => {
            model_overlaps(windows, threshold, MIN_SILENT_OVERLAP_SEC)
                .into_iter()
                .filter_map(|(start, end)| {
                    let heard = heard(turns, start, end);
                    if heard.len() != 1 {
                        return None;
                    }
                    let second = if mode == SilentOverlaps::ModelNeighbour {
                        neighbour_speaker(turns, start, end, &heard)
                    } else {
                        local_speaker(windows, turns, start, end, &heard)
                    };
                    second.map(|s| (start, end, s))
                })
                .collect()
        }
    };
    for (start, end, speaker) in found {
        let busy = turns
            .iter()
            .any(|t| t.speaker == speaker && t.start < end && start < t.end);
        if !busy {
            let mut silent = turn(speaker, start, end, String::new(), Some(Vec::new()));
            silent.flags.push(OVERLAP_FLAG.into());
            turns.push(silent);
        }
    }
    turns.sort_by(|a, b| a.start.total_cmp(&b.start));
}

// Говорящие, чьи ходы с текстом заходят в промежуток.
fn heard(turns: &[Turn], start: f64, end: f64) -> Vec<u32> {
    let mut speakers: Vec<u32> = turns
        .iter()
        .filter(|t| !t.text.trim().is_empty() && t.start < end && start < t.end)
        .map(|t| t.speaker)
        .collect();
    speakers.sort_unstable();
    speakers.dedup();
    speakers
}

fn cluster_id(label: usize) -> u32 {
    u32::try_from(label).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    const ONE: [f32; 7] = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    const BOTH: [f32; 7] = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    const TWO: [f32; 7] = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];

    // Говорящий 0 звучит 0..4, говорящий 1 с 4 с; модель слышит обоих в 3..4.
    fn window() -> PowersetWindow {
        PowersetWindow {
            first_frame_sec: 0.5,
            frame_sec: 1.0,
            frames: vec![ONE, ONE, ONE, BOTH, TWO, TWO, TWO],
        }
    }

    fn shape(turns: &[Turn]) -> Vec<(u32, f64, f64, &str, bool)> {
        turns
            .iter()
            .map(|t| {
                let overlap = t.flags.iter().any(|f| f == OVERLAP_FLAG);
                (t.speaker, t.start, t.end, t.text.as_str(), overlap)
            })
            .collect()
    }

    fn run(mode: SilentOverlaps) -> Vec<Turn> {
        let mut turns = transcript(&[(0, 0.0, 4.0, "раз"), (1, 4.0, 7.0, "два")]).turns;
        add_silent_overlaps(&mut turns, &[], &[window()], mode, 0.5);
        turns
    }

    #[test]
    fn both_model_ways_give_the_silent_turn_to_the_second_voice() {
        for mode in [
            SilentOverlaps::ModelNeighbour,
            SilentOverlaps::ModelLocalSpeakers,
        ] {
            assert_eq!(
                shape(&run(mode)),
                [
                    (0, 0.0, 4.0, "раз", false),
                    (1, 3.0, 4.0, "", true),
                    (1, 4.0, 7.0, "два", false)
                ],
                "{mode:?}"
            );
        }
    }

    #[test]
    fn off_adds_nothing() {
        assert_eq!(run(SilentOverlaps::Off).len(), 2);
    }

    #[test]
    fn spans_mode_uses_crossing_spans_of_two_voices() {
        let mut turns = transcript(&[(0, 0.0, 5.0, "раз")]).turns;
        let labeled = [(0.0, 5.0, 0), (2.0, 3.0, 1)];
        add_silent_overlaps(&mut turns, &labeled, &[], SilentOverlaps::Spans, 0.5);
        assert_eq!(
            shape(&turns),
            [(0, 0.0, 5.0, "раз", false), (1, 2.0, 3.0, "", true)]
        );
    }
}
