use serde::{Deserialize, Serialize};

use nixyon_ort::ChildAdultFeatures;

/// Для чего клип в замере «ребёнок или взрослый».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureRole {
    /// Обучение и выбор штрафа и порога (train speechocean762).
    Train,
    /// Проверка, говорящие не из обучения (test speechocean762).
    Test,
    /// Дополнительная проверка на других наборах.
    Extra,
}

/// Клип с разметкой «ребёнок или взрослый» и признаками голоса обеих моделей.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureClip {
    pub role: FeatureRole,
    pub set: String,
    pub speaker: String,
    pub child: bool,
    pub duration_sec: f64,
    pub redimnet2: Vec<f32>,
    pub age_gender_hidden: Vec<f32>,
}

impl FeatureClip {
    pub fn features(&self, kind: ChildAdultFeatures) -> &[f32] {
        match kind {
            ChildAdultFeatures::Redimnet2 => &self.redimnet2,
            ChildAdultFeatures::AgeGenderHidden => &self.age_gender_hidden,
        }
    }
}
