use nixyon_metrics::{
    ErrorCount, char_errors, word_errors, word_errors_lenient, word_errors_relaxed,
    word_errors_with_digits,
};
use serde::Serialize;

use crate::ClipDer;

/// Итог распознавания одного клипа.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipResult {
    /// Набор, из которого клип; пусто, если набор не указан.
    pub dataset: String,
    pub id: String,
    pub duration_sec: f64,
    pub processing_sec: f64,
    pub reference: String,
    pub hypothesis: String,
    pub speakers: u32,
    pub word_errors: usize,
    pub words: usize,
    pub char_errors: usize,
    pub chars: usize,
    /// Слова как есть: регистр и знаки препинания тоже считаются.
    pub strict_word_errors: usize,
    pub strict_words: usize,
    /// Числа с обеих сторон приведены к цифрам: «46 966» и «сорок шесть тысяч…» совпадают.
    pub digit_word_errors: usize,
    pub digit_words: usize,
    /// Мягкий WER: слова через дефис склеены, междометия не считаются.
    pub lenient_word_errors: usize,
    pub lenient_words: usize,
    /// Без ошибок записи: числа, единицы, дефисы, междометия и разговорные формы не
    /// считаются. По ним клип попадает в худшие.
    pub relaxed_word_errors: usize,
    pub relaxed_words: usize,
    /// Текст не получен: ошибка декодера или движка; распознанный текст пустой.
    pub failure: Option<String>,
    /// Сколько людей говорит в клипе на самом деле, если это известно.
    pub expected_speakers: Option<u32>,
    /// DER, если у клипа есть разметка «кто и когда говорит».
    pub der: Option<ClipDer>,
}

/// Что распознал конвейер в одном клипе.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipHypothesis {
    pub text: String,
    pub speakers: u32,
}

impl ClipResult {
    pub fn new(
        id: &str,
        reference: &str,
        duration_sec: f64,
        processing_sec: f64,
        outcome: Result<ClipHypothesis, String>,
    ) -> Self {
        let (hypothesis, speakers, failure) = match outcome {
            Ok(h) => (h.text, h.speakers, None),
            Err(e) => (String::new(), 0, Some(e)),
        };
        let words = word_errors(reference, &hypothesis);
        let chars = char_errors(reference, &hypothesis);
        let strict = strict_word_errors(reference, &hypothesis);
        let with_digits = word_errors_with_digits(reference, &hypothesis);
        let lenient = word_errors_lenient(reference, &hypothesis);
        let relaxed = word_errors_relaxed(reference, &hypothesis);
        Self {
            dataset: String::new(),
            id: id.to_owned(),
            duration_sec,
            processing_sec,
            reference: reference.to_owned(),
            speakers,
            word_errors: words.errors,
            words: words.reference_len,
            char_errors: chars.errors,
            chars: chars.reference_len,
            strict_word_errors: strict.errors,
            strict_words: strict.reference_len,
            digit_word_errors: with_digits.errors,
            digit_words: with_digits.reference_len,
            lenient_word_errors: lenient.errors,
            lenient_words: lenient.reference_len,
            relaxed_word_errors: relaxed.errors,
            relaxed_words: relaxed.reference_len,
            failure,
            hypothesis,
            expected_speakers: None,
            der: None,
        }
    }

    /// Тот же итог с DER по разметке спикеров.
    pub fn with_der(mut self, der: Option<ClipDer>) -> Self {
        self.der = der;
        self
    }

    /// Тот же итог с набором и настоящим числом голосов.
    pub fn from_dataset(mut self, dataset: &str, expected_speakers: Option<u32>) -> Self {
        dataset.clone_into(&mut self.dataset);
        self.expected_speakers = expected_speakers;
        self
    }

    /// Найдено столько голосов, сколько есть на самом деле. None: число неизвестно или
    /// текст не получен.
    pub fn speakers_match(&self) -> Option<bool> {
        let expected = self.expected_speakers?;
        self.failure.is_none().then_some(self.speakers == expected)
    }

    pub fn is_empty(&self) -> bool {
        self.hypothesis.trim().is_empty()
    }

    /// Модель записала число цифрами, а эталон Common Voice пишет числа словами.
    pub fn has_digits(&self) -> bool {
        self.hypothesis.chars().any(|c| c.is_ascii_digit())
    }

    pub fn wer(&self) -> Option<f64> {
        (self.words > 0).then(|| self.word_errors as f64 / self.words as f64)
    }

    /// Доля ошибок без ошибок записи: по ней клип стоит среди худших.
    pub fn relaxed_wer(&self) -> Option<f64> {
        (self.relaxed_words > 0)
            .then(|| self.relaxed_word_errors as f64 / self.relaxed_words as f64)
    }

    /// Клип с настоящими ошибками: они есть и по главному WER, и без ошибок записи.
    /// Только такой клип идёт в худшие.
    pub fn has_real_errors(&self) -> bool {
        self.word_errors > 0 && self.relaxed_word_errors > 0
    }
}

fn strict_word_errors(reference: &str, hypothesis: &str) -> ErrorCount {
    let reference: Vec<&str> = reference.split_whitespace().collect();
    let hypothesis: Vec<&str> = hypothesis.split_whitespace().collect();
    ErrorCount {
        errors: nixyon_metrics::edit_distance(&reference, &hypothesis),
        reference_len: reference.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(text: &str, speakers: u32) -> Result<ClipHypothesis, String> {
        Ok(ClipHypothesis {
            text: text.into(),
            speakers,
        })
    }

    #[test]
    fn punctuation_and_case_count_only_in_the_strict_score() {
        let r = ClipResult::new(
            "a",
            "Никогда со мной этого не бывало.",
            3.8,
            0.5,
            heard("никогда со мной этого не бывало", 1),
        );
        assert_eq!((r.word_errors, r.words), (0, 6));
        assert_eq!((r.strict_word_errors, r.strict_words), (2, 6));
        assert_eq!(r.char_errors, 0);
        assert_eq!(r.wer(), Some(0.0));
    }

    #[test]
    fn a_failed_clip_counts_every_reference_word_as_missed() {
        let r = ClipResult::new("a", "раз два", 1.0, 0.1, Err("нет речи".into()));
        assert_eq!((r.word_errors, r.words), (2, 2));
        assert!(r.is_empty());
        assert_eq!(r.failure.as_deref(), Some("нет речи"));
        assert_eq!(r.speakers, 0);
    }

    #[test]
    fn digits_in_the_hypothesis_are_noticed() {
        assert!(ClipResult::new("a", "две тысячи", 1.0, 0.1, heard("2000", 1)).has_digits());
        assert!(!ClipResult::new("a", "две", 1.0, 0.1, heard("две", 1)).has_digits());
    }

    #[test]
    fn a_reference_without_words_has_no_wer() {
        assert_eq!(
            ClipResult::new("a", "…", 1.0, 0.1, heard("", 1)).wer(),
            None
        );
    }
}
