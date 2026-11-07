use crate::SpeechLanguage;

/// Язык записи по ответам на отрезки: английский, только если за него больше половины
/// отрезков. Иначе, в том числе без отрезков и при ничьей, русский: основной язык
/// приложения, и его модель на чужой речи даёт пометку неразборчивой речи, а не догадку.
pub fn language_vote(votes: &[Option<SpeechLanguage>]) -> SpeechLanguage {
    let en = votes
        .iter()
        .filter(|v| **v == Some(SpeechLanguage::En))
        .count();
    if en * 2 > votes.len() {
        SpeechLanguage::En
    } else {
        SpeechLanguage::Ru
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SpeechLanguage::{En, Ru};

    #[test]
    fn english_needs_more_than_half() {
        assert_eq!(
            language_vote(&[Some(En), Some(En), Some(En), Some(Ru), Some(Ru)]),
            En
        );
        assert_eq!(
            language_vote(&[Some(En), Some(En), None, Some(Ru), Some(Ru)]),
            Ru
        );
        assert_eq!(language_vote(&[Some(En), Some(Ru)]), Ru);
        assert_eq!(language_vote(&[Some(En)]), En);
    }

    #[test]
    fn unclear_result_is_russian() {
        assert_eq!(language_vote(&[]), Ru);
        assert_eq!(language_vote(&[None, None, None]), Ru);
    }
}
