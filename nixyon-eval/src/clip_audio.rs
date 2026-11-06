use std::io::Cursor;
use std::path::Path;

use nixyon_audio::{QualityReport, WavWriter, convert_reader_to_wav16k};

use crate::{DecodedAudio, decode_flac_mono, decode_mp3_mono, decode_ogg_mono};

/// Приводит звук клипа к WAV 16 кГц моно тем же путём, что импорт в приложении (с
/// выравниванием громкости). mp3, Ogg и FLAC сначала декодируются, WAV идёт как есть. Файлы
/// .opus из open_stt внутри Ogg Vorbis, их читает тот же декодер Ogg.
pub fn clip_to_wav16k(audio: &Path, wav: &Path) -> Result<QualityReport, String> {
    let ext = audio
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("wav") => {
            let file = std::fs::File::open(audio).map_err(|e| format!("wav: {e}"))?;
            convert_reader_to_wav16k(std::io::BufReader::new(file), wav)
                .map_err(|e| format!("16 кГц: {e}"))
        }
        Some("mp3") => through_wav(
            decode_mp3_mono(audio).map_err(|e| format!("mp3: {e}"))?,
            wav,
        ),
        Some("flac") => through_wav(
            decode_flac_mono(audio).map_err(|e| format!("flac: {e}"))?,
            wav,
        ),
        Some("ogg" | "opus") => through_wav(
            decode_ogg_mono(audio).map_err(|e| format!("ogg: {e}"))?,
            wav,
        ),
        _ => Err(format!("формат {} не поддерживается", audio.display())),
    }
}

fn through_wav(decoded: DecodedAudio, wav: &Path) -> Result<QualityReport, String> {
    let mut writer = WavWriter::new(Cursor::new(Vec::new()), decoded.sample_rate)
        .map_err(|e| format!("wav: {e}"))?;
    writer
        .write(&decoded.samples)
        .map_err(|e| format!("wav: {e}"))?;
    let bytes = writer
        .finish(decoded.sample_rate)
        .map_err(|e| format!("wav: {e}"))?
        .into_inner();
    convert_reader_to_wav16k(Cursor::new(bytes), wav).map_err(|e| format!("16 кГц: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name)
    }

    #[test]
    fn mp3_ogg_flac_and_wav_give_the_same_length_at_16_khz() {
        let out =
            std::env::temp_dir().join(format!("nixyon-clip-audio-{}.wav", std::process::id()));
        let lengths: Vec<f64> = [
            "common_voice_ru_38615634.mp3",
            "vorbis_38615634.opus",
            "flac_38615634.flac",
            "wav_38615634.wav",
        ]
        .iter()
        .map(|name| clip_to_wav16k(&data(name), &out).unwrap().duration_sec)
        .collect();
        let _ = std::fs::remove_file(&out);
        for d in &lengths {
            assert!((d - 3.816).abs() < 0.01, "{lengths:?}");
        }
    }

    #[test]
    fn an_unknown_format_is_refused() {
        let out = std::env::temp_dir().join("nixyon-clip-audio-unknown.wav");
        assert!(clip_to_wav16k(&data("common_voice_ru_38615634.json"), &out).is_err());
    }
}
