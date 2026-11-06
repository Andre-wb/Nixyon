use crate::weighted_vector_mean::weighted_vector_mean;
use crate::{ChildAdult, OrtError};

/// Ребёнок ли спикер по признакам его кусков речи: признаки усредняются с весом по длине
/// куска, как говорящий при обучении, и классификатор отвечает один раз. Some(true):
/// ребёнок, Some(false): взрослый, None: кусков нет или классификатор не уверен.
pub fn speaker_child_verdict(
    model: &ChildAdult,
    pieces: &[(Vec<f32>, f64)],
) -> Result<Option<bool>, OrtError> {
    match weighted_vector_mean(pieces) {
        Some(mean) => model.verdict(&mean),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChildAdultFeatures;

    // Ребёнок, когда первый признак больше нуля; уверенность от 0,9.
    fn model() -> ChildAdult {
        ChildAdult {
            features: ChildAdultFeatures::AgeGenderHidden,
            mean: vec![0.0, 0.0],
            scale: vec![1.0, 1.0],
            weights: vec![10.0, 0.0],
            bias: 0.0,
            confidence: 0.9,
        }
    }

    #[test]
    fn a_long_piece_outweighs_a_short_one() {
        let pieces = [(vec![1.0, 0.0], 9.0), (vec![-1.0, 0.0], 1.0)];
        assert_eq!(
            speaker_child_verdict(&model(), &pieces).unwrap(),
            Some(true)
        );
        let pieces = [(vec![1.0, 0.0], 1.0), (vec![-1.0, 0.0], 9.0)];
        assert_eq!(
            speaker_child_verdict(&model(), &pieces).unwrap(),
            Some(false)
        );
    }

    #[test]
    fn no_pieces_or_an_unsure_answer_give_no_verdict() {
        assert_eq!(speaker_child_verdict(&model(), &[]).unwrap(), None);
        let pieces = [(vec![0.01, 0.0], 1.0)];
        assert_eq!(speaker_child_verdict(&model(), &pieces).unwrap(), None);
    }

    #[test]
    fn pieces_of_the_wrong_length_are_an_error() {
        assert!(speaker_child_verdict(&model(), &[(vec![1.0], 1.0)]).is_err());
    }
}
