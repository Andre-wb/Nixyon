use std::collections::BTreeMap;

use serde::Serialize;

use crate::{ChildAdultScore, FeatureClip, FeatureRole, TrainedChildAdult, speaker_centroid};

/// Допустимая доля взрослых, названных детьми, и наименьшая доля найденных детей, при
/// которых подпись «ребёнок» можно показывать.
pub const MAX_ADULTS_AS_CHILD: f64 = 0.02;
pub const MIN_CHILDREN_FOUND: f64 = 0.8;
/// Имя строки со всеми говорящими test из всех наборов: по ней решается приёмка.
pub const ALL_TEST_SET: &str = "test, все наборы";

/// Строка итога: набор, по клипам или по говорящим, ответы по классам.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChildAdultRow {
    pub set: String,
    pub role: FeatureRole,
    pub speakers: bool,
    pub score: ChildAdultScore,
}

/// Итог классификатора на всех наборах при выбранном пороге. Обучающие клипы и говорящие
/// считаются по ответам моделей, которые их не видели; у остальных наборов говорящий оценивается по
/// сводным признакам своих клипов, как спикер в приложении.
pub fn evaluate_child_adult(
    trained: &TrainedChildAdult,
    clips: &[FeatureClip],
) -> Result<Vec<ChildAdultRow>, String> {
    let model = &trained.model;
    let confidence = model.confidence;
    let train: Vec<(bool, f32)> = clips
        .iter()
        .filter(|c| c.role == FeatureRole::Train)
        .map(|c| c.child)
        .zip(trained.held_out.iter().copied())
        .collect();
    let mut rows = vec![
        ChildAdultRow {
            set: "train (отложенные части)".into(),
            role: FeatureRole::Train,
            speakers: false,
            score: ChildAdultScore::new(&train, confidence),
        },
        ChildAdultRow {
            set: "train (отложенные части, здесь выбран порог)".into(),
            role: FeatureRole::Train,
            speakers: true,
            score: ChildAdultScore::new(&trained.held_out_speakers, confidence),
        },
    ];
    let mut sets: BTreeMap<(FeatureRole, &str), Vec<&FeatureClip>> = BTreeMap::new();
    for c in clips.iter().filter(|c| c.role != FeatureRole::Train) {
        sets.entry((c.role, c.set.as_str())).or_default().push(c);
    }
    let mut all_test = Vec::new();
    let mut set_rows = Vec::new();
    for ((role, set), clips) in sets {
        let mut items = Vec::with_capacity(clips.len());
        let mut speakers: BTreeMap<&str, Vec<&FeatureClip>> = BTreeMap::new();
        for c in &clips {
            let p = model
                .child_probability(c.features(model.features))
                .map_err(|e| e.to_string())?;
            items.push((c.child, p));
            speakers.entry(c.speaker.as_str()).or_default().push(c);
        }
        let mut by_speaker = Vec::with_capacity(speakers.len());
        for group in speakers.values() {
            let parts: Vec<(&[f32], f64)> = group
                .iter()
                .map(|c| (c.features(model.features), c.duration_sec))
                .collect();
            let Some(centroid) = speaker_centroid(model.features, &parts) else {
                continue;
            };
            let p = model
                .child_probability(&centroid)
                .map_err(|e| e.to_string())?;
            by_speaker.push((group[0].child, p));
        }
        if role == FeatureRole::Test {
            all_test.extend(by_speaker.iter().copied());
        }
        for (speakers, items) in [(false, items), (true, by_speaker)] {
            set_rows.push(ChildAdultRow {
                set: set.to_string(),
                role,
                speakers,
                score: ChildAdultScore::new(&items, confidence),
            });
        }
    }
    if !all_test.is_empty() {
        rows.push(ChildAdultRow {
            set: ALL_TEST_SET.into(),
            role: FeatureRole::Test,
            speakers: true,
            score: ChildAdultScore::new(&all_test, confidence),
        });
    }
    rows.extend(set_rows);
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nixyon_ort::{ChildAdult, ChildAdultFeatures};

    fn clip(role: FeatureRole, speaker: &str, child: bool, x: f32) -> FeatureClip {
        FeatureClip {
            role,
            set: "s".into(),
            speaker: speaker.into(),
            child,
            duration_sec: 1.0,
            redimnet2: vec![x],
            age_gender_hidden: vec![x],
        }
    }

    #[test]
    fn rows_cover_held_out_training_and_each_set_by_clips_and_speakers() {
        let trained = TrainedChildAdult {
            model: ChildAdult {
                features: ChildAdultFeatures::AgeGenderHidden,
                mean: vec![0.0],
                scale: vec![1.0],
                weights: vec![4.0],
                bias: 0.0,
                confidence: 0.9,
            },
            l2: 0.1,
            l2_losses: vec![],
            held_out: vec![0.95],
            held_out_speakers: vec![(true, 0.95)],
            confidence_found: true,
        };
        let clips = [
            clip(FeatureRole::Train, "t", true, 1.0),
            clip(FeatureRole::Test, "a", false, -1.0),
            clip(FeatureRole::Test, "a", false, 0.1),
            clip(FeatureRole::Test, "c", true, 1.0),
        ];
        let rows = evaluate_child_adult(&trained, &clips).unwrap();
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[2].set, ALL_TEST_SET);
        assert_eq!(rows[2].score, rows[4].score);
        assert_eq!(rows[0].score.children_found, 1);
        let by_clip = &rows[3].score;
        assert_eq!((by_clip.adults, by_clip.adults_found), (2, 1));
        let by_speaker = &rows[4].score;
        assert!(rows[4].speakers);
        // Среднее признаков говорящего a: -0,45, p = 0,14, уверенность 0,86 ниже порога.
        assert_eq!((by_speaker.adults, by_speaker.adults_found), (1, 0));
        assert_eq!(by_speaker.children_found, 1);
    }
}
