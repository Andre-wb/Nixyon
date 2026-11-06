use nixyon_ort::VoiceTraits;

use crate::VoiceGender;

/// Клип с разметкой и оценкой модели.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredVoiceClip {
    pub speaker: String,
    pub gender: Option<VoiceGender>,
    pub child: Option<bool>,
    pub duration_sec: f64,
    pub traits: VoiceTraits,
}

impl ScoredVoiceClip {
    /// Вероятность женского голоса при выборе только из двух: класс «ребёнок» в выборе пола
    /// не участвует, иначе у детей пол не определялся бы совсем.
    pub fn female_share(&self) -> f32 {
        female_share(&self.traits)
    }
}

pub fn female_share(t: &VoiceTraits) -> f32 {
    let sum = t.female + t.male;
    if sum > 0.0 { t.female / sum } else { 0.5 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traits(female: f32, male: f32, child: f32) -> VoiceTraits {
        VoiceTraits {
            age_years: 30.0,
            female,
            male,
            child,
        }
    }

    #[test]
    fn the_child_class_does_not_take_part_in_the_gender_choice() {
        assert!((female_share(&traits(0.2, 0.1, 0.7)) - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn zero_female_and_male_give_an_even_share() {
        assert_eq!(female_share(&traits(0.0, 0.0, 1.0)), 0.5);
    }
}
