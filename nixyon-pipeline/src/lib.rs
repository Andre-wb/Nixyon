//! Вторая фаза конвейера: слова из токенов, склейка ходов, статистика, правки спикеров, сборка расшифровки, статистика встречи, поиск по тексту.

mod build_transcript;
mod edit;
mod error;
mod meeting_stats;
mod merge_centers;
mod neural_labels;
mod neural_turns;
mod pipeline_error;
mod pipeline_options;
mod resegment;
mod sample_names;
mod sample_threshold;
mod search;
mod sentence_fix;
mod speaker_count;
mod speaker_labels;
mod speaker_names;
mod speaker_voice_pieces;
mod stats;
mod transcript_meta;
mod turns;
mod voice_match;
mod window_label;
mod word_cut;
mod word_speakers;
mod words;

#[cfg(test)]
mod test_support;

pub use build_transcript::{build_transcript, build_transcript_with_centers};
pub use edit::{
    MAX_TURN_TEXT_CHARS, delete_turn, edit_turn_text, merge_speakers, rename_speaker,
    set_turn_emotion,
};
pub use error::EditError;
pub use meeting_stats::{MONOLOGUE_SEC, MeetingStats, SpeakerSummary, meeting_stats};
pub use merge_centers::merge_centers;
pub use pipeline_error::PipelineError;
pub use pipeline_options::PipelineOptions;
pub use sample_names::SpeakerCenter;
pub use sample_threshold::SAMPLE_THRESHOLD;
pub use search::{MatchKind, SearchHit, search};
pub use speaker_count::SpeakerCount;
pub use speaker_labels::{
    LONG_SEGMENT_SEC, MAX_AUTO_SPEAKERS, MIN_SPEAKER_SEC, SpeakerLabels, label_segments,
};
pub use speaker_names::{apply_names, clean_name, intro_names, name_is_grounded};
pub use speaker_voice_pieces::{
    SpeakerPieces, VOICE_PIECE_MIN_SEC, VOICE_PIECES_MAX_SEC, speaker_voice_pieces,
};
pub use stats::recompute_speaker_stats;
pub use transcript_meta::TranscriptMeta;
pub use turns::{MAX_PAUSE_TO_GLUE_SEC, glue_turns};
pub use voice_match::VoiceMatch;
pub use window_label::WindowLabel;
pub use words::tokens_to_words;
