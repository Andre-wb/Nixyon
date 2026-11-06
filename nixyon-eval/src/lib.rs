//! Замер качества на наборе размеченных записей: перевод разметки и расшифровки
//! в типы метрик, счёт по записи, таблица по набору.

mod balanced_weights;
mod binary_score;
mod child_adult_evaluation;
mod child_adult_score;
mod child_adult_training;
mod child_age;
mod childmandarin_corpus;
mod clip_audio;
mod clip_der;
mod clip_files;
mod clip_line;
mod clip_result;
mod clip_sample;
mod clip_tag;
mod combo;
mod combo_analysis;
mod compressed_audio;
mod cv_clip;
mod dataset_index;
mod dataset_kind;
mod excluded_clips;
mod feature_clip;
mod hypothesis;
mod labeled_voice_clip;
mod logistic_fit;
mod min_speaker;
mod progress_meter;
mod quality_plan;
mod quality_summary;
mod quality_table;
mod record_score;
mod reference_source;
mod render_child_adult;
mod render_voice_traits;
mod run_history;
mod samromur_corpus;
mod score;
mod scored_voice_clip;
mod segmenter;
mod speaker_centroid;
mod speaker_folds;
mod speech_spans;
mod speechocean_corpus;
mod sps_corpus;
mod table;
mod truth;
mod truth_turn;
mod unscored_record;
mod voice_gender;
mod voice_library;
mod voice_model;
mod voice_samples;
mod voice_traits_report;
mod wer_totals;
mod word_substitutions;

pub use balanced_weights::balanced_weights;
pub use binary_score::{BinaryScore, CONFIDENCE_THRESHOLDS, ClassScore, ConfidenceRow};
pub use child_adult_evaluation::{
    ALL_TEST_SET, ChildAdultRow, MAX_ADULTS_AS_CHILD, MIN_CHILDREN_FOUND, evaluate_child_adult,
};
pub use child_adult_score::{ChildAdultScore, choose_confidence};
pub use child_adult_training::{FOLDS, L2_GRID, TrainedChildAdult, train_child_adult};
pub use childmandarin_corpus::{ChildMandarinError, read_childmandarin};
pub use clip_audio::clip_to_wav16k;
pub use clip_der::ClipDer;
pub use clip_files::{AUDIO_DIR, CLIPS_DIR, SetClip, SetFiles, WORST_AUDIO, write_set_clips};
pub use clip_line::{ClipLine, EditCell};
pub use clip_result::{ClipHypothesis, ClipResult};
pub use clip_sample::pick_clips;
pub use clip_tag::{ClipTag, clip_tags};
pub use combo::Combo;
pub use combo_analysis::{ComboAnalysis, ComboModels, analyze_combos};
pub use compressed_audio::{
    CompressedAudioError, DecodedAudio, decode_flac_mono, decode_mp3_mono, decode_ogg_mono,
};
pub use cv_clip::{CvClip, CvClipError, CvClipFiles, list_cv_clips};
pub use dataset_index::{
    Dataset, DatasetError, DatasetInfo, IndexClip, audio_path, is_valid_dataset_id, list_datasets,
    load_dataset,
};
pub use dataset_kind::DatasetKind;
pub use excluded_clips::ExcludedClips;
pub use feature_clip::{FeatureClip, FeatureRole};
pub use hypothesis::{
    hypothesis_named_spans, hypothesis_overlaps, hypothesis_spans, hypothesis_text,
    hypothesis_utterances,
};
pub use labeled_voice_clip::LabeledVoiceClip;
pub use logistic_fit::fit_logistic;
pub use min_speaker::min_speaker_sec;
pub use progress_meter::ProgressMeter;
pub use quality_plan::{PlanItem, parse_plan_item};
pub use quality_summary::{ErrorTotal, GroupScore, QualitySummary, WORST_CLIPS, worse_first};
pub use quality_table::{DatasetScore, RunInfo, render_quality};
pub use record_score::RecordScore;
pub use reference_source::ReferenceSource;
pub use render_child_adult::render_child_adult;
pub use render_voice_traits::render_voice_traits;
pub use run_history::{KEEP_RUNS, archive_run, is_commit, is_run_name, run_name};
pub use samromur_corpus::{SamromurClip, SamromurError, read_samromur};
pub use score::{RecordRun, score_record};
pub use scored_voice_clip::{ScoredVoiceClip, female_share};
pub use segmenter::Segmenter;
pub use speaker_centroid::speaker_centroid;
pub use speaker_folds::speaker_folds;
pub use speech_spans::{SPEECH_GAP_SEC, speech_spans};
pub use speechocean_corpus::{SpeechoceanError, read_speechocean, read_speechocean_split};
pub use sps_corpus::{SpsCorpusError, read_sps_corpus};
pub use table::render_table;
pub use truth::Truth;
pub use truth_turn::TruthTurn;
pub use unscored_record::{LARGE_SPEAKER_SEC, UnscoredRecord};
pub use voice_gender::VoiceGender;
pub use voice_library::library_from_meeting;
pub use voice_model::VoiceModel;
pub use voice_samples::load_voice_samples;
pub use voice_traits_report::VoiceTraitsReport;
pub use wer_totals::{SetsTotal, WerTotals, wer_totals};
pub use word_substitutions::{SubstitutionCount, TOP_SUBSTITUTIONS, top_substitutions};
