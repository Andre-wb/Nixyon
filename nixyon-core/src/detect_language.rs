use crate::{
    CancelFlag, EngineError, LanguageDetector, SpeechLanguage, language_clips, language_vote,
};

/// Язык записи: `detector` слушает отрезки из `language_clips`, решает `language_vote`.
/// `samples`: вся запись 16 кГц, `bounds`: её куски речи в сэмплах.
pub fn detect_language(
    detector: &dyn LanguageDetector,
    samples: &[f32],
    bounds: &[(usize, usize)],
    cancel: &CancelFlag,
) -> Result<SpeechLanguage, EngineError> {
    let mut votes = Vec::new();
    for (from, to) in language_clips(bounds, samples.len()) {
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        votes.push(detector.language(&samples[from..to])?);
    }
    Ok(language_vote(&votes))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Английский на отрезках, которые начинаются в первой половине записи.
    struct ByPosition;

    impl LanguageDetector for ByPosition {
        fn language(&self, clip: &[f32]) -> Result<Option<SpeechLanguage>, EngineError> {
            Ok(Some(if clip[0] < 0.5 {
                SpeechLanguage::En
            } else {
                SpeechLanguage::Ru
            }))
        }
    }

    fn record() -> Vec<f32> {
        (0..100).map(|i| i as f32 / 100.0).collect()
    }

    #[test]
    fn majority_of_clips_decides() {
        let samples = record();
        // Отрезки в частях 1, 2, 3 английские, в 4 и 5 русские.
        let bounds = [(1, 9), (21, 29), (41, 49), (61, 69), (81, 89)];
        let lang = detect_language(&ByPosition, &samples, &bounds, &CancelFlag::new()).unwrap();
        assert_eq!(lang, SpeechLanguage::En);
        let lang = detect_language(&ByPosition, &samples, &bounds[1..], &CancelFlag::new());
        assert_eq!(lang.unwrap(), SpeechLanguage::Ru);
    }

    #[test]
    fn cancel_stops_detection() {
        let cancel = CancelFlag::new();
        cancel.cancel();
        let err = detect_language(&ByPosition, &record(), &[(1, 9)], &cancel).unwrap_err();
        assert!(matches!(err, EngineError::Cancelled));
    }
}
