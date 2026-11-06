//! Метрики качества расшифровки: DER по времени спикеров, WER и CER по тексту,
//! cpWER по тексту с учётом спикеров, точность и полнота пометки перекрытий,
//! имена спикеров по образцам голоса.
//! Ошибки по нескольким записям складываются, доля считается от суммы.

mod colloquial;
mod comparable_numbers;
mod cpwer;
mod der;
mod der_breakdown;
mod digit_tails;
mod edit_distance;
mod error_count;
mod lenient_words;
mod metrics_error;
mod naming;
mod naming_score;
mod normalize;
mod number_units;
mod number_word;
mod numbers_to_digits;
mod overlap_detection;
mod overlap_score;
mod particles;
mod relaxed_words;
mod scored_intervals;
mod speaker_span;
mod spoken_symbols;
mod utterance;
mod wer;
mod word_alignment;
mod word_edit;

pub use comparable_numbers::digit_words;
pub use cpwer::cp_word_errors;
pub use der::{DER_COLLAR_SEC, diarization_errors};
pub use der_breakdown::DerBreakdown;
pub use edit_distance::edit_distance;
pub use error_count::ErrorCount;
pub use lenient_words::{is_filler, lenient_words};
pub use metrics_error::MetricsError;
pub use naming::{NamedSpan, naming_score};
pub use naming_score::NamingScore;
pub use normalize::normalize_words;
pub use number_word::{NumberWord, number_word};
pub use numbers_to_digits::numbers_to_digits;
pub use overlap_detection::overlap_detection;
pub use overlap_score::OverlapScore;
pub use relaxed_words::relaxed_words;
pub use speaker_span::SpeakerSpan;
pub use utterance::Utterance;
pub use wer::{
    char_errors, word_errors, word_errors_lenient, word_errors_relaxed, word_errors_with_digits,
};
pub use word_alignment::{word_alignment, word_alignment_relaxed};
pub use word_edit::WordEdit;
