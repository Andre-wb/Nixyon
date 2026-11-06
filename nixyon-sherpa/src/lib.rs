//! Движок распознавания для ноутбука на крейте sherpa-onnx: VAD, GigaAM, эмбеддинги голоса,
//! нейросегментация pyannote, потоковое распознавание для живого текста, шумоподавление GTCRN.
//! Параметры совпадают с Kotlin-плагином, чтобы ноутбук и телефон давали одинаковый результат.

mod analyze;
mod analyze_recording;
mod asr_groups;
mod blocks;
mod clip_spans;
mod config;
mod decoding;
mod denoise;
mod engine;
mod error;
mod long_pieces;
mod manifest;
mod models;
mod music_check;
mod padded_bounds;
mod parallel;
mod pyannote_segmenter;
mod read_wave;
mod sample_embedding;
mod segment_embeddings;
mod sherpa_voice;
mod speech_regions;
mod split_by_pieces;
mod streaming;
mod vad_denoiser;
mod voice_spans;

pub use analyze::{analyze, bounds_seconds, recognize, recognize_bounds, speech_bounds};
pub use analyze_recording::{analyze_default, analyze_recording};
pub use config::threads;
pub use decoding::{BEAM_PATHS, Decoding, HOTWORD_SCORE};
pub use denoise::denoise_wav;
pub use engine::SherpaEngine;
pub use error::SherpaError;

pub use manifest::{ModelManifest, ModelRole};
pub use models::SherpaModels;
pub use parallel::Parallel;
pub use pyannote_segmenter::PyannoteSegmenter;
pub use read_wave::read_samples;
pub use sample_embedding::{sample_embedding, sample_embedding_of};
pub use segment_embeddings::embed_segments;
pub use sherpa_voice::SherpaVoice;
pub use streaming::{LiveText, StreamModel};
pub use voice_spans::{embed_spans, segment_speech, voice_spans};
