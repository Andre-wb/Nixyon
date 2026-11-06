use nixyon_ort::ChildAdultFeatures;

/// Признаки говорящего по признакам его клипов, так же, как приложение сводит куски
/// спикера: эмбеддинги ReDimNet2 усредняются и приводятся к длине 1, `hidden_states`
/// усредняются с весом по длине клипа. None, если нечего сводить.
pub fn speaker_centroid(features: ChildAdultFeatures, clips: &[(&[f32], f64)]) -> Option<Vec<f32>> {
    let dim = clips.first()?.0.len();
    if dim == 0 || clips.iter().any(|(v, _)| v.len() != dim) {
        return None;
    }
    let weight = |w: f64| match features {
        ChildAdultFeatures::Redimnet2 => 1.0,
        ChildAdultFeatures::AgeGenderHidden => w,
    };
    let total: f64 = clips.iter().map(|(_, w)| weight(*w)).sum();
    if total <= 0.0 || !total.is_finite() {
        return None;
    }
    let mut sum = vec![0.0f64; dim];
    for (v, w) in clips {
        for (s, x) in sum.iter_mut().zip(v.iter()) {
            *s += f64::from(*x) * weight(*w);
        }
    }
    let scale = match features {
        ChildAdultFeatures::Redimnet2 => sum.iter().map(|s| s * s).sum::<f64>().sqrt(),
        ChildAdultFeatures::AgeGenderHidden => total,
    };
    if scale <= f64::EPSILON || !scale.is_finite() {
        return None;
    }
    Some(sum.into_iter().map(|s| (s / scale) as f32).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redimnet_embeddings_are_averaged_to_unit_length() {
        let c = speaker_centroid(
            ChildAdultFeatures::Redimnet2,
            &[(&[1.0, 0.0], 1.0), (&[0.0, 1.0], 9.0)],
        )
        .unwrap();
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!((c[0] - h).abs() < 1e-6 && (c[1] - h).abs() < 1e-6);
    }

    #[test]
    fn hidden_states_are_averaged_by_duration() {
        let c = speaker_centroid(
            ChildAdultFeatures::AgeGenderHidden,
            &[(&[0.0], 1.0), (&[4.0], 3.0)],
        )
        .unwrap();
        assert_eq!(c, [3.0]);
    }

    #[test]
    fn nothing_or_mixed_lengths_give_nothing() {
        assert_eq!(speaker_centroid(ChildAdultFeatures::Redimnet2, &[]), None);
        assert_eq!(
            speaker_centroid(
                ChildAdultFeatures::Redimnet2,
                &[(&[1.0], 1.0), (&[1.0, 0.0], 1.0)]
            ),
            None
        );
    }
}
