/// Тип голоса по полу из разметки набора.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VoiceGender {
    Female,
    Male,
}

impl VoiceGender {
    pub fn name(self) -> &'static str {
        match self {
            Self::Female => "female",
            Self::Male => "male",
        }
    }
}
