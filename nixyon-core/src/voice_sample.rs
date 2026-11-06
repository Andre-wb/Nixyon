/// Образец голоса участника: имя и эмбеддинг голоса по его речи. Звук образца
/// не хранится.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSample {
    pub name: String,
    pub embedding: Vec<f32>,
}
