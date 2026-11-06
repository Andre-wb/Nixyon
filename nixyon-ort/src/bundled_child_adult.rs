use crate::age_gender::HIDDEN_DIM;
use crate::{ChildAdult, ChildAdultFeatures, OrtError};

/// Веса `child_adult_eval` на признаках `hidden_states` audEERING: обучены на train
/// speechocean762, Samrómur Children и Samrómur.
const WEIGHTS: &str = include_str!("../data/child_adult_age_gender_hidden.json");

/// Классификатор «ребёнок или взрослый», вшитый в библиотеку. Ошибка, если веса испорчены
/// или посчитаны не на `hidden_states` модели audEERING.
pub fn bundled_child_adult() -> Result<ChildAdult, OrtError> {
    let model = ChildAdult::from_json(WEIGHTS)?;
    if model.features != ChildAdultFeatures::AgeGenderHidden || model.dim() != HIDDEN_DIM {
        return Err(OrtError::BadManifest(format!(
            "вшитый классификатор на признаках {} длины {}, ждали {} длины {HIDDEN_DIM}",
            model.features.key(),
            model.dim(),
            ChildAdultFeatures::AgeGenderHidden.key()
        )));
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_weights_read_the_hidden_states_of_the_age_gender_model() {
        let model = bundled_child_adult().unwrap();
        assert_eq!(model.features, ChildAdultFeatures::AgeGenderHidden);
        assert_eq!(model.dim(), HIDDEN_DIM);
    }

    #[test]
    fn the_bundled_classifier_answers_on_features_of_its_length() {
        let model = bundled_child_adult().unwrap();
        let p = model.child_probability(&model.mean).unwrap();
        assert!((0.0..=1.0).contains(&p));
        assert!(model.child_probability(&[0.0; 3]).is_err());
    }
}
