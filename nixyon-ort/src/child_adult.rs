use serde::{Deserialize, Serialize};

use crate::{ChildAdultFeatures, OrtError};

/// Классификатор «ребёнок или взрослый» по признакам голоса: логистическая регрессия на
/// стандартизованных признаках. Ответ с уверенностью ниже `confidence` считается
/// «не определено».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildAdult {
    pub features: ChildAdultFeatures,
    /// Признак перед весами приводится к (x - mean) / scale.
    pub mean: Vec<f32>,
    pub scale: Vec<f32>,
    pub weights: Vec<f32>,
    pub bias: f32,
    /// Наименьшая уверенность, max(p, 1 - p), при которой ответ выдаётся.
    pub confidence: f32,
}

impl ChildAdult {
    pub fn from_json(text: &str) -> Result<Self, OrtError> {
        let model: Self =
            serde_json::from_str(text).map_err(|e| OrtError::BadManifest(e.to_string()))?;
        model.check()?;
        Ok(model)
    }

    pub fn to_json(&self) -> Result<String, OrtError> {
        self.check()?;
        serde_json::to_string_pretty(self).map_err(|e| OrtError::BadManifest(e.to_string()))
    }

    pub fn dim(&self) -> usize {
        self.weights.len()
    }

    /// Вероятность, что голос детский.
    pub fn child_probability(&self, features: &[f32]) -> Result<f32, OrtError> {
        if features.len() != self.dim() {
            return Err(OrtError::BadOutput(format!(
                "признаков {}, классификатор ждёт {}",
                features.len(),
                self.dim()
            )));
        }
        let mut z = f64::from(self.bias);
        for (((x, m), s), w) in features
            .iter()
            .zip(&self.mean)
            .zip(&self.scale)
            .zip(&self.weights)
        {
            z += f64::from((x - m) / s) * f64::from(*w);
        }
        if !z.is_finite() {
            return Err(OrtError::BadOutput("признаки не конечные числа".into()));
        }
        Ok((1.0 / (1.0 + (-z).exp())) as f32)
    }

    /// Some(true): ребёнок, Some(false): взрослый, None: не определено.
    pub fn verdict(&self, features: &[f32]) -> Result<Option<bool>, OrtError> {
        let p = self.child_probability(features)?;
        Ok((p.max(1.0 - p) >= self.confidence).then_some(p >= 0.5))
    }

    fn check(&self) -> Result<(), OrtError> {
        let dim = self.dim();
        let finite = |v: &[f32]| v.iter().all(|x| x.is_finite());
        let ok = dim > 0
            && self.mean.len() == dim
            && self.scale.len() == dim
            && finite(&self.mean)
            && finite(&self.weights)
            && self.scale.iter().all(|s| s.is_finite() && *s > 0.0)
            && self.bias.is_finite()
            && (0.5..=1.0).contains(&self.confidence);
        if ok {
            Ok(())
        } else {
            Err(OrtError::BadManifest(
                "классификатор «ребёнок или взрослый»: длины, масштабы или порог неверные".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(confidence: f32) -> ChildAdult {
        ChildAdult {
            features: ChildAdultFeatures::Redimnet2,
            mean: vec![1.0, 0.0],
            scale: vec![2.0, 1.0],
            weights: vec![1.0, -1.0],
            bias: 0.0,
            confidence,
        }
    }

    #[test]
    fn features_are_standardized_before_the_weights() {
        // (3 - 1) / 2 * 1 + (0 - 0) * -1 = 1.
        let p = model(0.5).child_probability(&[3.0, 0.0]).unwrap();
        assert!((p - 1.0 / (1.0 + (-1.0f32).exp())).abs() < 1e-6);
    }

    #[test]
    fn below_the_confidence_the_answer_is_unknown() {
        let m = model(0.9);
        assert_eq!(m.verdict(&[1.0, 0.0]).unwrap(), None);
        assert_eq!(m.verdict(&[11.0, 0.0]).unwrap(), Some(true));
        assert_eq!(m.verdict(&[1.0, 5.0]).unwrap(), Some(false));
    }

    #[test]
    fn a_wrong_feature_length_is_an_error() {
        assert!(model(0.5).child_probability(&[1.0]).is_err());
    }

    #[test]
    fn json_round_trips_and_broken_models_are_refused() {
        let m = model(0.8);
        assert_eq!(ChildAdult::from_json(&m.to_json().unwrap()).unwrap(), m);
        let mut zero_scale = model(0.8);
        zero_scale.scale[0] = 0.0;
        assert!(zero_scale.to_json().is_err());
        let mut short = model(0.8);
        short.mean.pop();
        assert!(short.to_json().is_err());
        assert!(model(0.4).to_json().is_err());
    }
}
