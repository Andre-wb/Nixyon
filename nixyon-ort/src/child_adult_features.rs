use serde::{Deserialize, Serialize};

/// На каких признаках голоса построен классификатор «ребёнок или взрослый».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildAdultFeatures {
    /// Эмбеддинг ReDimNet2-B6 (роль redimnet2), той же модели, что узнаёт спикеров.
    Redimnet2,
    /// Выход `hidden_states` модели audEERING.
    AgeGenderHidden,
}

impl ChildAdultFeatures {
    pub const ALL: [Self; 2] = [Self::Redimnet2, Self::AgeGenderHidden];

    pub fn key(self) -> &'static str {
        match self {
            Self::Redimnet2 => "redimnet2",
            Self::AgeGenderHidden => "age_gender_hidden",
        }
    }
}
