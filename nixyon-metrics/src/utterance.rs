/// Реплика спикера. Реплики передаются в порядке времени: текст одного спикера
/// склеивается в этом порядке.
#[derive(Debug, Clone, PartialEq)]
pub struct Utterance {
    pub speaker: u32,
    pub text: String,
}
