use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::path::Path;

use crate::{
    AudioError, GainApplier, GainCurve, LevelAnalyzer, QualityMeter, QualityReport, Resampler16k,
    WavReader, WavWriter,
};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;
const READ_FRAMES: usize = 16_384;

/// Переводит WAV любой поддерживаемой частоты и числа каналов в WAV 16 кГц моно PCM16,
/// выравнивает громкость (тихий собеседник подтягивается к громкому, иначе VAD теряет
/// его реплики) и меряет качество исходной записи. Два прохода по файлу: запись не
/// держится в памяти. Пишет во временные файлы рядом и переименовывает в конце, чтобы
/// при ошибке не остался недописанный файл.
pub fn convert_to_wav16k(input: &Path, output: &Path) -> Result<QualityReport, AudioError> {
    convert_reader_to_wav16k(File::open(input)?, output)
}

/// То же для записи, которую читают по порядку из потока, например расшифровывая на лету.
pub fn convert_reader_to_wav16k(
    input: impl Read,
    output: &Path,
) -> Result<QualityReport, AudioError> {
    let raw = output.with_extension("wav.raw.part");
    let tmp = output.with_extension("wav.part");
    let result = convert(input, &raw, &tmp);
    // Промежуточный файл не нужен ни при успехе, ни при ошибке; его может и не быть.
    let _ = std::fs::remove_file(&raw);
    match result {
        Ok(report) => {
            std::fs::rename(&tmp, output)?;
            Ok(report)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn convert(input: impl Read, raw: &Path, tmp: &Path) -> Result<QualityReport, AudioError> {
    let (report, curve) = resample_and_measure(input, raw)?;
    apply_gain(raw, tmp, &curve)?;
    Ok(report)
}

// Первый проход: 16 кГц моно во временный файл, качество и уровни по окнам.
fn resample_and_measure(
    input: impl Read,
    raw: &Path,
) -> Result<(QualityReport, GainCurve), AudioError> {
    let mut reader = WavReader::new(BufReader::new(input))?;
    let mut resampler = Resampler16k::new(reader.format().sample_rate)?;
    let mut writer = WavWriter::new(BufWriter::new(File::create(raw)?), TARGET_SAMPLE_RATE)?;
    let mut meter = QualityMeter::new(TARGET_SAMPLE_RATE);
    let mut levels = LevelAnalyzer::new(TARGET_SAMPLE_RATE);
    let mut read = Vec::with_capacity(READ_FRAMES);
    let mut out = Vec::new();
    loop {
        read.clear();
        let frames = reader.read_mono(READ_FRAMES, &mut read)?;
        out.clear();
        if frames == 0 {
            break;
        }
        resampler.push(&read, &mut out)?;
        meter.push(&out);
        levels.push(&out);
        writer.write(&out)?;
    }
    resampler.finish(&mut out)?;
    meter.push(&out);
    levels.push(&out);
    writer.write(&out)?;
    finish(writer)?;
    Ok((meter.finish(), levels.finish()))
}

// Второй проход: усиление по кривой уровней.
fn apply_gain(raw: &Path, tmp: &Path, curve: &GainCurve) -> Result<(), AudioError> {
    let mut reader = WavReader::new(BufReader::new(File::open(raw)?))?;
    let mut writer = WavWriter::new(BufWriter::new(File::create(tmp)?), TARGET_SAMPLE_RATE)?;
    let mut applier = GainApplier::new(curve);
    let mut buf = Vec::with_capacity(READ_FRAMES);
    loop {
        buf.clear();
        if reader.read_mono(READ_FRAMES, &mut buf)? == 0 {
            break;
        }
        applier.apply(&mut buf);
        writer.write(&buf)?;
    }
    finish(writer)
}

fn finish(writer: WavWriter<BufWriter<File>>) -> Result<(), AudioError> {
    let file = writer.finish(TARGET_SAMPLE_RATE)?;
    file.into_inner()
        .map_err(|e| AudioError::Io(e.into_error()))?
        .sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TempDir, WavBytes, read_all, sine};

    #[test]
    fn stereo_48k_becomes_mono_16k_with_same_duration() {
        let dir = TempDir::new("convert");
        let input = dir.path().join("in.wav");
        let pcm: Vec<i16> = sine(48_000, 440.0, 0.3, 2.0)
            .iter()
            .flat_map(|s| {
                let v = (s * 32_767.0) as i16;
                [v, v]
            })
            .collect();
        std::fs::write(&input, WavBytes::pcm16(48_000, 2, &pcm).build()).unwrap();

        let output = dir.path().join("out.wav");
        let report = convert_to_wav16k(&input, &output).unwrap();
        assert!((report.duration_sec - 2.0).abs() < 1e-9);
        assert!(report.issues().is_empty());

        let (format, samples) = read_all(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!((format.sample_rate, format.channels), (16_000, 1));
        assert_eq!(samples.len(), 32_000);
        assert!(!dir.path().join("out.wav.part").exists());
        assert!(!dir.path().join("out.wav.raw.part").exists());
    }

    #[test]
    fn quiet_part_is_louder_after_conversion_and_report_keeps_original_level() {
        let dir = TempDir::new("convert-level");
        let input = dir.path().join("in.wav");
        // Громкий собеседник 2 с, пауза 0,5 с, тихий 3 с; в паузе слабый шум, как в жизни.
        let mut samples = sine(16_000, 300.0, 0.5, 2.0);
        samples.extend((0..8_000).map(|i| if i % 2 == 0 { 0.0003 } else { -0.0003 }));
        samples.extend(sine(16_000, 200.0, 0.01, 3.0));
        let pcm: Vec<i16> = samples.iter().map(|s| (s * 32_767.0) as i16).collect();
        std::fs::write(&input, WavBytes::pcm16(16_000, 1, &pcm).build()).unwrap();

        let output = dir.path().join("out.wav");
        let report = convert_to_wav16k(&input, &output).unwrap();
        // Отчёт о качестве по исходной записи: самое громкое окно около -9 дБ.
        assert!((report.loudest_window_dbfs + 9.0).abs() < 1.0, "{report:?}");
        let (_, out) = read_all(&std::fs::read(&output).unwrap()).unwrap();
        // Дальше окрестности громкой речи: 4,2-5,5 с.
        let quiet_after = crate::dbfs(crate::rms(&out[67_200..88_000]));
        assert!(
            quiet_after > -30.0,
            "тихая часть после выравнивания {quiet_after:.1} дБ"
        );
    }

    #[test]
    fn stream_input_gives_the_same_file_as_path_input() {
        let dir = TempDir::new("convert-stream");
        let input = dir.path().join("in.wav");
        let pcm: Vec<i16> = sine(22_050, 440.0, 0.3, 1.0)
            .iter()
            .map(|s| (s * 32_767.0) as i16)
            .collect();
        let bytes = WavBytes::pcm16(22_050, 1, &pcm).build();
        std::fs::write(&input, &bytes).unwrap();
        let (a, b) = (dir.path().join("a.wav"), dir.path().join("b.wav"));
        convert_to_wav16k(&input, &a).unwrap();
        convert_reader_to_wav16k(&bytes[..], &b).unwrap();
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    }

    #[test]
    fn broken_input_leaves_no_output_files() {
        let dir = TempDir::new("convert-broken");
        let input = dir.path().join("in.wav");
        std::fs::write(&input, b"RIFF....WAVEjunk").unwrap();
        let output = dir.path().join("out.wav");
        assert!(convert_to_wav16k(&input, &output).is_err());
        assert!(!output.exists());
        assert!(!dir.path().join("out.wav.part").exists());
        assert!(!dir.path().join("out.wav.raw.part").exists());
    }

    #[test]
    fn missing_input_is_an_io_error() {
        let dir = TempDir::new("convert-missing");
        let err = convert_to_wav16k(&dir.path().join("nope.wav"), &dir.path().join("out.wav"))
            .unwrap_err();
        assert!(matches!(err, AudioError::Io(_)));
    }
}
