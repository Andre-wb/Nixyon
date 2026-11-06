use std::path::PathBuf;

use crate::VoiceGender;

/// Клип с разметкой говорящего для замера пола и возраста. Пустое поле: в наборе не указано
/// или указано так, что для замера не годится.
#[derive(Debug, Clone, PartialEq)]
pub struct LabeledVoiceClip {
    /// Говорящий внутри набора: по нему клипы сводятся в одного человека.
    pub speaker: String,
    pub audio: PathBuf,
    pub gender: Option<VoiceGender>,
    pub child: Option<bool>,
}
