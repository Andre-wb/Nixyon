use std::io::Read;

use crate::{AudioError, SampleEncoding, WavFormat};

const FORMAT_PCM: u16 = 1;
const FORMAT_FLOAT: u16 = 3;
const FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const MAX_CHANNELS: u16 = 8;
const MIN_SAMPLE_RATE: u32 = 8_000;
const MAX_SAMPLE_RATE: u32 = 192_000;
// Больше этого заголовочные куски (LIST, fact и т. п.) не читаются: защита от
// файла, который объявляет гигантский кусок перед данными.
const MAX_SKIPPED_CHUNK: u32 = 1 << 20;
// Так пишут размер данных программы, которые пишут WAV потоком и не знают длину заранее.
const UNKNOWN_DATA_SIZE: u32 = u32::MAX;

/// Читает WAV потоком и отдаёт моно: каналы усредняются.
pub struct WavReader<R: Read> {
    input: R,
    format: WavFormat,
    /// None: размер данных неизвестен, читать до конца файла.
    remaining_bytes: Option<u64>,
    raw: Vec<u8>,
}

impl<R: Read> WavReader<R> {
    pub fn new(mut input: R) -> Result<Self, AudioError> {
        let mut riff = [0u8; 12];
        read_header(&mut input, &mut riff)?;
        if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
            return Err(AudioError::NotWav("нет заголовка RIFF/WAVE"));
        }
        let mut format = None;
        loop {
            let mut chunk = [0u8; 8];
            read_header(&mut input, &mut chunk)?;
            let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            match &chunk[0..4] {
                b"fmt " => format = Some(read_fmt(&mut input, size)?),
                b"data" => {
                    let format =
                        format.ok_or(AudioError::NotWav("данные раньше описания формата"))?;
                    let remaining_bytes = (size != UNKNOWN_DATA_SIZE).then_some(u64::from(size));
                    return Ok(Self {
                        input,
                        format,
                        remaining_bytes,
                        raw: Vec::new(),
                    });
                }
                _ => skip_chunk(&mut input, size)?,
            }
        }
    }

    pub fn format(&self) -> WavFormat {
        self.format
    }

    /// Читает до `max_frames` кадров и дописывает их моно в `out`. Возвращает число
    /// прочитанных кадров; 0 значит конец данных. Неполный кадр в конце файла отбрасывается.
    pub fn read_mono(
        &mut self,
        max_frames: usize,
        out: &mut Vec<f32>,
    ) -> Result<usize, AudioError> {
        let frame_bytes = self.format.frame_bytes();
        let mut want = max_frames.saturating_mul(frame_bytes);
        if let Some(remaining) = self.remaining_bytes {
            want = want.min(usize::try_from(remaining).unwrap_or(usize::MAX));
        }
        want -= want % frame_bytes;
        if want == 0 {
            return Ok(0);
        }
        self.raw.resize(want, 0);
        let got = read_up_to(&mut self.input, &mut self.raw)?;
        let frames = got / frame_bytes;
        if let Some(remaining) = self.remaining_bytes.as_mut() {
            *remaining -= got as u64;
            if got < want {
                // Файл короче, чем объявлено в заголовке: дальше читать нечего.
                *remaining = 0;
            }
        }
        let channels = usize::from(self.format.channels);
        out.reserve(frames);
        for frame in self.raw[..frames * frame_bytes].chunks_exact(frame_bytes) {
            let sum: f32 = frame
                .chunks_exact(self.format.encoding.bytes())
                .map(|s| decode(self.format.encoding, s))
                .sum();
            out.push(sum / channels as f32);
        }
        Ok(frames)
    }
}

fn decode(encoding: SampleEncoding, bytes: &[u8]) -> f32 {
    match encoding {
        SampleEncoding::Pcm16 => f32::from(i16::from_le_bytes([bytes[0], bytes[1]])) / 32_768.0,
        SampleEncoding::Float32 => {
            let v = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            if v.is_finite() { v } else { 0.0 }
        }
    }
}

fn read_fmt<R: Read>(input: &mut R, size: u32) -> Result<WavFormat, AudioError> {
    if !(16..=MAX_SKIPPED_CHUNK).contains(&size) {
        return Err(AudioError::BadFormat("размер описания формата"));
    }
    let mut buf = vec![0u8; size as usize];
    read_header(input, &mut buf)?;
    if size % 2 == 1 {
        skip_bytes(input, 1)?;
    }
    let mut format_tag = u16::from_le_bytes([buf[0], buf[1]]);
    let channels = u16::from_le_bytes([buf[2], buf[3]]);
    let sample_rate = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let bits = u16::from_le_bytes([buf[14], buf[15]]);
    if format_tag == FORMAT_EXTENSIBLE {
        // Реальный формат лежит в первых двух байтах GUID подформата.
        if buf.len() < 26 {
            return Err(AudioError::BadFormat(
                "неполное расширенное описание формата",
            ));
        }
        format_tag = u16::from_le_bytes([buf[24], buf[25]]);
    }
    let encoding = match (format_tag, bits) {
        (FORMAT_PCM, 16) => SampleEncoding::Pcm16,
        (FORMAT_FLOAT, 32) => SampleEncoding::Float32,
        _ => return Err(AudioError::UnsupportedFormat { format_tag, bits }),
    };
    if channels == 0 || channels > MAX_CHANNELS {
        return Err(AudioError::BadFormat("число каналов"));
    }
    if !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&sample_rate) {
        return Err(AudioError::BadFormat("частота дискретизации"));
    }
    Ok(WavFormat {
        sample_rate,
        channels,
        encoding,
    })
}

fn skip_chunk<R: Read>(input: &mut R, size: u32) -> Result<(), AudioError> {
    if size > MAX_SKIPPED_CHUNK {
        return Err(AudioError::BadFormat("слишком большой служебный кусок"));
    }
    // Куски WAV выровнены по 2 байта.
    skip_bytes(input, u64::from(size) + u64::from(size % 2))
}

fn skip_bytes<R: Read>(input: &mut R, n: u64) -> Result<(), AudioError> {
    let skipped = std::io::copy(&mut input.take(n), &mut std::io::sink())?;
    if skipped < n {
        return Err(AudioError::NotWav("файл оборвался в заголовке"));
    }
    Ok(())
}

fn read_header<R: Read>(input: &mut R, buf: &mut [u8]) -> Result<(), AudioError> {
    input.read_exact(buf).map_err(|e| match e.kind() {
        std::io::ErrorKind::UnexpectedEof => AudioError::NotWav("файл оборвался в заголовке"),
        _ => AudioError::Io(e),
    })
}

fn read_up_to<R: Read>(input: &mut R, buf: &mut [u8]) -> Result<usize, AudioError> {
    let mut filled = 0;
    while filled < buf.len() {
        match input.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{WavBytes, read_all};

    #[test]
    fn reads_pcm16_mono() {
        let bytes = WavBytes::pcm16(16_000, 1, &[0, 16_384, -32_768, 32_767]).build();
        let (format, samples) = read_all(&bytes).unwrap();
        assert_eq!(format.sample_rate, 16_000);
        assert_eq!(samples, [0.0, 0.5, -1.0, 32_767.0 / 32_768.0]);
    }

    #[test]
    fn stereo_is_averaged_to_mono() {
        let bytes = WavBytes::pcm16(48_000, 2, &[16_384, 0, -16_384, -16_384]).build();
        let (_, samples) = read_all(&bytes).unwrap();
        assert_eq!(samples, [0.25, -0.5]);
    }

    #[test]
    fn reads_float32_and_extensible_formats() {
        let floats = WavBytes::float32(44_100, 1, &[0.25, -0.75]).build();
        assert_eq!(read_all(&floats).unwrap().1, [0.25, -0.75]);
        let ext = WavBytes::pcm16(16_000, 1, &[16_384]).extensible().build();
        assert_eq!(read_all(&ext).unwrap().1, [0.5]);
    }

    #[test]
    fn nan_and_infinity_in_float_wav_become_silence() {
        let bytes = WavBytes::float32(16_000, 1, &[f32::NAN, f32::INFINITY, 0.5]).build();
        assert_eq!(read_all(&bytes).unwrap().1, [0.0, 0.0, 0.5]);
    }

    #[test]
    fn unknown_chunks_including_odd_sized_are_skipped() {
        let bytes = WavBytes::pcm16(16_000, 1, &[16_384])
            .extra_chunk(b"LIST", 3)
            .build();
        assert_eq!(read_all(&bytes).unwrap().1, [0.5]);
    }

    #[test]
    fn data_shorter_than_declared_is_read_up_to_end() {
        let mut bytes = WavBytes::pcm16(16_000, 1, &[1, 2, 3]).build();
        bytes.truncate(bytes.len() - 3);
        assert_eq!(read_all(&bytes).unwrap().1.len(), 1);
    }

    #[test]
    fn unknown_data_size_reads_until_end_of_file() {
        let bytes = WavBytes::pcm16(16_000, 1, &[1, 2, 3])
            .unknown_data_size()
            .build();
        assert_eq!(read_all(&bytes).unwrap().1.len(), 3);
    }

    #[test]
    fn rejects_garbage_and_truncated_headers() {
        assert!(matches!(read_all(b""), Err(AudioError::NotWav(_))));
        assert!(matches!(
            read_all(b"RIFF\0\0\0\0WAVX"),
            Err(AudioError::NotWav(_))
        ));
        let good = WavBytes::pcm16(16_000, 1, &[1]).build();
        for cut in [4, 12, 20, 30, 40] {
            assert!(read_all(&good[..cut]).is_err(), "обрезано до {cut}");
        }
    }

    #[test]
    fn rejects_unsupported_and_invalid_formats() {
        let pcm8 = WavBytes::pcm16(16_000, 1, &[1]).bits(8).build();
        assert!(matches!(
            read_all(&pcm8),
            Err(AudioError::UnsupportedFormat {
                format_tag: 1,
                bits: 8
            })
        ));
        let no_channels = WavBytes::pcm16(16_000, 0, &[]).build();
        assert!(matches!(
            read_all(&no_channels),
            Err(AudioError::BadFormat(_))
        ));
        let low_rate = WavBytes::pcm16(100, 1, &[1]).build();
        assert!(matches!(read_all(&low_rate), Err(AudioError::BadFormat(_))));
    }

    #[test]
    fn huge_unknown_chunk_is_rejected_without_reading_it() {
        let bytes = WavBytes::pcm16(16_000, 1, &[1])
            .huge_chunk_before_data()
            .build();
        assert!(matches!(read_all(&bytes), Err(AudioError::BadFormat(_))));
    }
}
