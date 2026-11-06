use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// Звук, сведённый в моно, с исходной частотой.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

#[derive(Debug, thiserror::Error)]
pub enum CompressedAudioError {
    #[error("не открывается: {0}")]
    Open(#[from] std::io::Error),
    #[error("формат не распознан или файл испорчен: {0}")]
    Format(SymphoniaError),
    #[error("в файле нет звуковой дорожки")]
    NoTrack,
    #[error("частота или число каналов меняются посреди файла")]
    SpecChanged,
    #[error("в файле нет ни одного целого кадра звука")]
    Empty,
}

/// Декодирует mp3 целиком и сводит каналы в моно средним. Битые кадры пропускаются,
/// как в проигрывателях: в наборах записей с телефонов они встречаются.
pub fn decode_mp3_mono(path: &Path) -> Result<DecodedAudio, CompressedAudioError> {
    decode_mono(path, "mp3")
}

/// То же для Ogg Vorbis. В open_stt такие файлы лежат с расширением .opus, поэтому
/// формат определяется по содержимому, а не по имени.
pub fn decode_ogg_mono(path: &Path) -> Result<DecodedAudio, CompressedAudioError> {
    decode_mono(path, "ogg")
}

/// То же для FLAC: в нём лежат Samrómur и Samrómur Children.
pub fn decode_flac_mono(path: &Path) -> Result<DecodedAudio, CompressedAudioError> {
    decode_mono(path, "flac")
}

fn decode_mono(path: &Path, extension: &str) -> Result<DecodedAudio, CompressedAudioError> {
    let stream = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(extension);
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(CompressedAudioError::Format)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or(CompressedAudioError::NoTrack)?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(CompressedAudioError::NoTrack)?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(CompressedAudioError::Format)?;
    let track_id = track.id;
    let mut spec: Option<(u32, usize)> = None;
    let mut interleaved = Vec::new();
    let mut samples = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(e) => return Err(CompressedAudioError::Format(e)),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_) | SymphoniaError::IoError(_)) => continue,
            Err(e) => return Err(CompressedAudioError::Format(e)),
        };
        let current = (decoded.spec().rate(), decoded.spec().channels().count());
        if *spec.get_or_insert(current) != current {
            return Err(CompressedAudioError::SpecChanged);
        }
        interleaved.resize(decoded.samples_interleaved(), 0.0f32);
        decoded.copy_to_slice_interleaved(&mut interleaved);
        push_mono(&interleaved, current.1, &mut samples);
    }
    match spec {
        Some((sample_rate, _)) if !samples.is_empty() => Ok(DecodedAudio {
            sample_rate,
            samples,
        }),
        _ => Err(CompressedAudioError::Empty),
    }
}

fn push_mono(interleaved: &[f32], channels: usize, out: &mut Vec<f32>) {
    if channels <= 1 {
        out.extend_from_slice(interleaved);
        return;
    }
    // Каналов в записи единицы, приведение к f32 без потерь.
    let scale = 1.0 / channels as f32;
    out.extend(
        interleaved
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() * scale),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_frames_are_averaged_into_mono() {
        let mut out = Vec::new();
        push_mono(&[1.0, 0.0, -0.5, -0.5], 2, &mut out);
        assert_eq!(out, [0.5, -0.5]);
    }

    #[test]
    fn mono_samples_pass_through() {
        let mut out = vec![0.25];
        push_mono(&[0.1, 0.2], 1, &mut out);
        assert_eq!(out, [0.25, 0.1, 0.2]);
    }

    #[test]
    fn a_common_voice_clip_decodes_to_speech_of_the_stated_length() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/common_voice_ru_38615634.mp3");
        let audio = decode_mp3_mono(&path).unwrap();
        let seconds = audio.samples.len() as f64 / f64::from(audio.sample_rate);
        // В метаданных клипа 3,816 с; декодер mp3 добавляет задержку кодера, не больше кадра.
        assert!((seconds - 3.816).abs() < 0.1, "{seconds}");
        assert!(audio.samples.iter().any(|s| s.abs() > 0.05));
    }

    #[test]
    fn ogg_vorbis_named_opus_decodes_to_the_clip_length() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/vorbis_38615634.opus");
        let audio = decode_ogg_mono(&path).unwrap();
        let seconds = audio.samples.len() as f64 / f64::from(audio.sample_rate);
        assert_eq!(audio.sample_rate, 16_000);
        assert!((seconds - 3.816).abs() < 0.05, "{seconds}");
        assert!(audio.samples.iter().any(|s| s.abs() > 0.05));
    }

    #[test]
    fn a_file_that_is_not_mp3_is_rejected() {
        let path = std::env::temp_dir().join(format!("nixyon-not-mp3-{}.mp3", std::process::id()));
        std::fs::write(&path, b"definitely not audio").unwrap();
        let result = decode_mp3_mono(&path);
        std::fs::remove_file(&path).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn a_missing_file_is_an_open_error() {
        let result = decode_mp3_mono(Path::new("/nonexistent/clip.mp3"));
        assert!(matches!(result, Err(CompressedAudioError::Open(_))));
    }
}
