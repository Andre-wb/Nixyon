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

/// Вырезает из записи кусок `[from_sec, to_sec)` и переводит его в WAV 16 кГц моно PCM16
/// с выравниванием громкости — тот же конвейер, что и для всей записи, но уровни меряются
/// по самому куску: одна фраза может быть тише записи в целом. Нужно, чтобы прослушать
/// реплику заново, не пересчитывая всю встречу. Конец записи дальше её длины обрезается,
/// пустой кусок (начало за концом записи) — ошибка.
pub fn slice_to_wav16k(
    input: &Path,
    output: &Path,
    from_sec: f64,
    to_sec: f64,
) -> Result<(), AudioError> {
    slice_reader_to_wav16k(File::open(input)?, output, from_sec, to_sec)
}

/// То же для записи, которую читают по порядку из потока.
pub fn slice_reader_to_wav16k(
    input: impl Read,
    output: &Path,
    from_sec: f64,
    to_sec: f64,
) -> Result<(), AudioError> {
    let (from, to) = slice_frames(from_sec, to_sec)?;
    let raw = output.with_extension("wav.raw.part");
    let tmp = output.with_extension("wav.part");
    let result = slice(input, &raw, &tmp, from, to);
    // Промежуточный файл не нужен ни при успехе, ни при ошибке; его может и не быть.
    let _ = std::fs::remove_file(&raw);
    match result {
        Ok(()) => {
            std::fs::rename(&tmp, output)?;
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Границы куска в сэмплах 16 кГц: время берётся в тех же секундах, что и в расшифровке.
fn slice_frames(from_sec: f64, to_sec: f64) -> Result<(u64, u64), AudioError> {
    if !(from_sec.is_finite() && to_sec.is_finite() && from_sec >= 0.0 && to_sec > from_sec) {
        return Err(AudioError::BadFormat("границы фрагмента"));
    }
    let from = (from_sec * f64::from(TARGET_SAMPLE_RATE)).round() as u64;
    let to = (to_sec * f64::from(TARGET_SAMPLE_RATE)).round() as u64;
    if to <= from {
        return Err(AudioError::BadFormat("пустой фрагмент"));
    }
    Ok((from, to))
}

fn slice(input: impl Read, raw: &Path, tmp: &Path, from: u64, to: u64) -> Result<(), AudioError> {
    let curve = resample_slice(input, raw, from, to)?;
    apply_gain(raw, tmp, &curve)
}

// Первый проход: 16 кГц моно во временный файл, но только сэмплы куска; уровни и
// кривая усиления — по куску, остальная запись не нужна и не дочитывается.
fn resample_slice(
    input: impl Read,
    raw: &Path,
    from: u64,
    to: u64,
) -> Result<GainCurve, AudioError> {
    let mut reader = WavReader::new(BufReader::new(input))?;
    let mut resampler = Resampler16k::new(reader.format().sample_rate)?;
    let mut writer = WavWriter::new(BufWriter::new(File::create(raw)?), TARGET_SAMPLE_RATE)?;
    let mut levels = LevelAnalyzer::new(TARGET_SAMPLE_RATE);
    let mut read = Vec::with_capacity(READ_FRAMES);
    let mut out = Vec::new();
    // Позиция сэмпла от начала записи в 16 кГц: ресемплинг сохраняет длину и положение.
    let mut index = 0u64;
    let mut written = 0u64;
    while index < to {
        read.clear();
        if reader.read_mono(READ_FRAMES, &mut read)? == 0 {
            break;
        }
        out.clear();
        resampler.push(&read, &mut out)?;
        let start = from.saturating_sub(index).min(out.len() as u64) as usize;
        let end = to.saturating_sub(index).min(out.len() as u64) as usize;
        if start < end {
            writer.write(&out[start..end])?;
            levels.push(&out[start..end]);
            written += u64::try_from(end - start).unwrap_or(u64::MAX);
        }
        index += out.len() as u64;
    }
    if written == 0 {
        return Err(AudioError::BadFormat("пустой фрагмент"));
    }
    finish(writer)?;
    Ok(levels.finish())
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

    #[test]
    fn slice_is_mono_16k_with_the_requested_duration() {
        let dir = TempDir::new("slice-duration");
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
        slice_to_wav16k(&input, &output, 0.5, 1.5).unwrap();
        let (format, samples) = read_all(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!((format.sample_rate, format.channels), (16_000, 1));
        assert_eq!(samples.len(), 16_000);
        assert!(!dir.path().join("out.wav.part").exists());
        assert!(!dir.path().join("out.wav.raw.part").exists());
    }

    #[test]
    fn slice_matches_the_same_part_of_the_whole_conversion() {
        let dir = TempDir::new("slice-match");
        let input = dir.path().join("in.wav");
        let pcm: Vec<i16> = sine(22_050, 440.0, 0.5, 1.5)
            .iter()
            .map(|s| (s * 32_767.0) as i16)
            .collect();
        std::fs::write(&input, WavBytes::pcm16(22_050, 1, &pcm).build()).unwrap();

        let whole = dir.path().join("whole.wav");
        let part = dir.path().join("part.wav");
        convert_to_wav16k(&input, &whole).unwrap();
        slice_to_wav16k(&input, &part, 0.4, 0.9).unwrap();
        let (_, all) = read_all(&std::fs::read(&whole).unwrap()).unwrap();
        let (_, sliced) = read_all(&std::fs::read(&part).unwrap()).unwrap();
        // Время фрагмента то же, что в расшифровке: сэмпл 6400 — это 0,4 с от начала.
        let expected = &all[6_400..14_400];
        assert_eq!(sliced.len(), expected.len());
        let worst = sliced
            .iter()
            .zip(expected)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 0.02, "расхождение с целым файлом {worst}");
    }

    #[test]
    fn slice_past_the_end_of_the_recording_is_cut_short() {
        let dir = TempDir::new("slice-end");
        let input = dir.path().join("in.wav");
        let pcm: Vec<i16> = sine(16_000, 440.0, 0.3, 2.0)
            .iter()
            .map(|s| (s * 32_767.0) as i16)
            .collect();
        std::fs::write(&input, WavBytes::pcm16(16_000, 1, &pcm).build()).unwrap();

        let output = dir.path().join("out.wav");
        slice_to_wav16k(&input, &output, 1.5, 3.0).unwrap();
        let (_, samples) = read_all(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!(samples.len(), 8_000);
    }

    #[test]
    fn slice_starting_past_the_end_leaves_no_files() {
        let dir = TempDir::new("slice-empty");
        let input = dir.path().join("in.wav");
        let pcm: Vec<i16> = sine(16_000, 440.0, 0.3, 1.0)
            .iter()
            .map(|s| (s * 32_767.0) as i16)
            .collect();
        std::fs::write(&input, WavBytes::pcm16(16_000, 1, &pcm).build()).unwrap();

        let output = dir.path().join("out.wav");
        let err = slice_to_wav16k(&input, &output, 5.0, 6.0).unwrap_err();
        assert!(matches!(err, AudioError::BadFormat(_)));
        assert!(!output.exists());
        assert!(!dir.path().join("out.wav.part").exists());
        assert!(!dir.path().join("out.wav.raw.part").exists());
    }

    #[test]
    fn bad_slice_bounds_are_rejected_before_reading() {
        let dir = TempDir::new("slice-bounds");
        let output = dir.path().join("out.wav");
        for (from, to) in [(1.0, 1.0), (2.0, 1.0), (-1.0, 1.0), (0.0, f64::NAN)] {
            let err = slice_reader_to_wav16k(&[][..], &output, from, to).unwrap_err();
            assert!(matches!(err, AudioError::BadFormat(_)), "{from}..{to}");
        }
        assert!(!output.exists());
    }

    #[test]
    fn missing_slice_input_is_an_io_error() {
        let dir = TempDir::new("slice-missing");
        let err = slice_to_wav16k(&dir.path().join("nope.wav"), &dir.path().join("out.wav"), 0.0, 1.0)
            .unwrap_err();
        assert!(matches!(err, AudioError::Io(_)));
    }
}
