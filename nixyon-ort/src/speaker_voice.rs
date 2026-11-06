use crate::{ChildAdult, OrtError, VoiceAnalysis, VoiceTraits, speaker_child_verdict};

/// Что модель audEERING и классификатор говорят о голосе спикера. None: ответа нет.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeakerVoice {
    pub child: Option<bool>,
    pub female: Option<bool>,
}

/// Голос спикера по его кускам речи (разбор куска и его длина в секундах). Ребёнок или
/// взрослый: классификатор по `hidden_states`, усреднённым с весом по длине. Пол: оценки
/// «женщина» и «мужчина» модели, усреднённые так же; при равенстве ответа нет. У детей
/// пол модель различает плохо, особенно у мальчиков, ответ всё равно выдаётся.
pub fn speaker_voice(
    model: &ChildAdult,
    pieces: &[(VoiceAnalysis, f64)],
) -> Result<SpeakerVoice, OrtError> {
    let hidden: Vec<(Vec<f32>, f64)> = pieces
        .iter()
        .map(|(a, sec)| (a.hidden_states.clone(), *sec))
        .collect();
    let traits: Vec<(VoiceTraits, f64)> = pieces.iter().map(|(a, sec)| (a.traits, *sec)).collect();
    let female = VoiceTraits::weighted_mean(&traits)
        .and_then(|t| (t.female != t.male).then_some(t.female > t.male));
    Ok(SpeakerVoice {
        child: speaker_child_verdict(model, &hidden)?,
        female,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChildAdultFeatures;

    fn model() -> ChildAdult {
        ChildAdult {
            features: ChildAdultFeatures::AgeGenderHidden,
            mean: vec![0.0],
            scale: vec![1.0],
            weights: vec![10.0],
            bias: 0.0,
            confidence: 0.9,
        }
    }

    fn piece(hidden: f32, female: f32, male: f32, sec: f64) -> (VoiceAnalysis, f64) {
        (
            VoiceAnalysis {
                traits: VoiceTraits {
                    age_years: 30.0,
                    female,
                    male,
                    child: 1.0 - female - male,
                },
                hidden_states: vec![hidden],
            },
            sec,
        )
    }

    #[test]
    fn the_longer_piece_decides_the_gender_and_the_age() {
        let pieces = [piece(1.0, 0.8, 0.1, 9.0), piece(-1.0, 0.1, 0.8, 1.0)];
        let voice = speaker_voice(&model(), &pieces).unwrap();
        assert_eq!(
            voice,
            SpeakerVoice {
                child: Some(true),
                female: Some(true)
            }
        );
        let pieces = [piece(1.0, 0.8, 0.1, 1.0), piece(-1.0, 0.1, 0.8, 9.0)];
        let voice = speaker_voice(&model(), &pieces).unwrap();
        assert_eq!(
            voice,
            SpeakerVoice {
                child: Some(false),
                female: Some(false)
            }
        );
    }

    #[test]
    fn no_pieces_or_a_tie_give_no_answer() {
        let empty = speaker_voice(&model(), &[]).unwrap();
        assert_eq!(
            empty,
            SpeakerVoice {
                child: None,
                female: None
            }
        );
        let tie = speaker_voice(&model(), &[piece(1.0, 0.4, 0.4, 1.0)]).unwrap();
        assert_eq!(tie.female, None);
    }
}
