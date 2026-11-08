//! Звук: WAV, ресемплинг, уровень, проверки тишины, клиппинга и слишком короткой записи.
//! Всё работает потоком: часовая запись не держится в памяти целиком.

mod convert;
mod error;
mod level;
mod level_normalizer;
mod quality;
mod resample;
mod wav_format;
mod wav_reader;
mod wav_writer;

#[cfg(test)]
mod test_support;

pub use convert::{
    TARGET_SAMPLE_RATE, convert_reader_to_wav16k, convert_reader_to_wav16k_and_plain,
    convert_to_wav16k, slice_reader_to_wav16k, slice_to_wav16k,
};
pub use error::AudioError;
pub use level::{dbfs, rms};
pub use level_normalizer::{GainApplier, GainCurve, LevelAnalyzer};
pub use quality::{QualityIssue, QualityMeter, QualityReport};
pub use resample::Resampler16k;
pub use wav_format::{SampleEncoding, WavFormat};
pub use wav_reader::WavReader;
pub use wav_writer::WavWriter;
