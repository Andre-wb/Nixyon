use std::collections::BTreeMap;

use nixyon_ort::VoiceTraits;
use serde::Serialize;

use crate::{BinaryScore, ScoredVoiceClip, VoiceGender};

/// Итог замера пола и возраста на одном наборе: по клипам и по говорящим. Говорящий
/// оценивается по среднему своих клипов с весом по длине, как спикер в приложении по своим
/// кускам.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VoiceTraitsReport {
    pub clips: usize,
    pub speakers: usize,
    pub audio_sec: f64,
    pub gender_clips: BinaryScore,
    pub gender_speakers: BinaryScore,
    /// Пол только у детей: у детских голосов он различим хуже.
    pub gender_child_clips: BinaryScore,
    pub child_clips: BinaryScore,
    pub child_speakers: BinaryScore,
}

impl VoiceTraitsReport {
    pub fn new(clips: &[ScoredVoiceClip]) -> Self {
        let speakers = by_speaker(clips);
        Self {
            clips: clips.len(),
            speakers: speakers.len(),
            audio_sec: clips.iter().map(|c| c.duration_sec).sum(),
            gender_clips: gender_score(clips.iter()),
            gender_speakers: gender_score(speakers.iter()),
            gender_child_clips: gender_score(clips.iter().filter(|c| c.child == Some(true))),
            child_clips: child_score(clips.iter()),
            child_speakers: child_score(speakers.iter()),
        }
    }
}

fn gender_score<'a>(clips: impl Iterator<Item = &'a ScoredVoiceClip>) -> BinaryScore {
    let items: Vec<(bool, f32)> = clips
        .filter_map(|c| {
            c.gender
                .map(|g| (g == VoiceGender::Female, c.female_share()))
        })
        .collect();
    BinaryScore::new("female", "male", &items)
}

fn child_score<'a>(clips: impl Iterator<Item = &'a ScoredVoiceClip>) -> BinaryScore {
    let items: Vec<(bool, f32)> = clips
        .filter_map(|c| c.child.map(|child| (child, c.traits.child)))
        .collect();
    BinaryScore::new("child", "adult", &items)
}

/// Один «клип» на говорящего: средняя оценка и разметка, если она у всех его клипов одна.
fn by_speaker(clips: &[ScoredVoiceClip]) -> Vec<ScoredVoiceClip> {
    let mut groups: BTreeMap<&str, Vec<&ScoredVoiceClip>> = BTreeMap::new();
    for clip in clips {
        groups.entry(clip.speaker.as_str()).or_default().push(clip);
    }
    groups
        .into_iter()
        .filter_map(|(speaker, clips)| {
            let weighted: Vec<(VoiceTraits, f64)> =
                clips.iter().map(|c| (c.traits, c.duration_sec)).collect();
            Some(ScoredVoiceClip {
                speaker: speaker.to_string(),
                gender: same(clips.iter().map(|c| c.gender)),
                child: same(clips.iter().map(|c| c.child)),
                duration_sec: clips.iter().map(|c| c.duration_sec).sum(),
                traits: VoiceTraits::weighted_mean(&weighted)?,
            })
        })
        .collect()
}

fn same<T: PartialEq>(mut values: impl Iterator<Item = Option<T>>) -> Option<T> {
    let first = values.next()??;
    values.all(|v| v.as_ref() == Some(&first)).then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(
        speaker: &str,
        gender: Option<VoiceGender>,
        child: Option<bool>,
        sec: f64,
        (female, male, kid): (f32, f32, f32),
    ) -> ScoredVoiceClip {
        ScoredVoiceClip {
            speaker: speaker.into(),
            gender,
            child,
            duration_sec: sec,
            traits: VoiceTraits {
                age_years: 30.0,
                female,
                male,
                child: kid,
            },
        }
    }

    const F: Option<VoiceGender> = Some(VoiceGender::Female);
    const M: Option<VoiceGender> = Some(VoiceGender::Male);

    #[test]
    fn a_speaker_is_judged_by_the_duration_weighted_mean_of_the_clips() {
        // Короткий клип ошибается, длинный нет: по говорящему ответ верный.
        let clips = [
            clip("a", F, Some(false), 1.0, (0.1, 0.9, 0.0)),
            clip("a", F, Some(false), 9.0, (0.8, 0.2, 0.0)),
        ];
        let r = VoiceTraitsReport::new(&clips);
        assert_eq!((r.gender_clips.total(), r.gender_clips.correct()), (2, 1));
        assert_eq!(
            (r.gender_speakers.total(), r.gender_speakers.correct()),
            (1, 1)
        );
        assert_eq!(r.speakers, 1);
        assert!((r.audio_sec - 10.0).abs() < 1e-9);
    }

    #[test]
    fn clips_without_a_label_are_left_out_of_that_score() {
        let clips = [
            clip("a", None, Some(true), 2.0, (0.1, 0.1, 0.8)),
            clip("b", M, None, 2.0, (0.1, 0.8, 0.1)),
        ];
        let r = VoiceTraitsReport::new(&clips);
        assert_eq!(r.gender_clips.total(), 1);
        assert_eq!(r.child_clips.total(), 1);
        assert_eq!(r.child_clips.a.correct, 1);
        assert_eq!(r.gender_child_clips.total(), 0);
    }

    #[test]
    fn a_speaker_with_conflicting_labels_gets_no_label() {
        let clips = [
            clip("a", F, Some(false), 1.0, (0.9, 0.1, 0.0)),
            clip("a", M, Some(false), 1.0, (0.9, 0.1, 0.0)),
        ];
        let r = VoiceTraitsReport::new(&clips);
        assert_eq!(r.gender_speakers.total(), 0);
        assert_eq!(r.child_speakers.total(), 1);
    }

    #[test]
    fn gender_of_children_is_scored_separately() {
        let clips = [
            clip("a", F, Some(true), 1.0, (0.2, 0.1, 0.7)),
            clip("b", M, Some(false), 1.0, (0.1, 0.9, 0.0)),
        ];
        let r = VoiceTraitsReport::new(&clips);
        assert_eq!(
            (r.gender_child_clips.total(), r.gender_child_clips.correct()),
            (1, 1)
        );
    }
}
