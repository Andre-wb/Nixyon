use crate::VoiceTraits;

/// Полный ответ модели audEERING на кусок звука: оценки пола и возраста и внутреннее
/// представление голоса (`hidden_states`), на котором можно строить свои классификаторы.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceAnalysis {
    pub traits: VoiceTraits,
    pub hidden_states: Vec<f32>,
}
