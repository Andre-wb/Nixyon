use std::io::{Seek, SeekFrom, Write};

use crate::AudioError;

const HEADER_BYTES: u32 = 44;

/// Пишет WAV PCM 16 бит моно потоком. Размеры в заголовке проставляются в `finish`.
pub struct WavWriter<W: Write + Seek> {
    output: W,
    data_bytes: u32,
    buf: Vec<u8>,
}

impl<W: Write + Seek> WavWriter<W> {
    pub fn new(mut output: W, sample_rate: u32) -> Result<Self, AudioError> {
        output.write_all(&header(sample_rate, 0))?;
        Ok(Self {
            output,
            data_bytes: 0,
            buf: Vec::new(),
        })
    }

    /// Сэмплы вне [-1, 1] обрезаются, NaN пишется тишиной.
    pub fn write(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        let added = u32::try_from(samples.len() * 2).map_err(|_| AudioError::TooLong)?;
        let total = self
            .data_bytes
            .checked_add(added)
            .filter(|&n| n <= u32::MAX - HEADER_BYTES)
            .ok_or(AudioError::TooLong)?;
        self.buf.clear();
        self.buf.reserve(samples.len() * 2);
        for &s in samples {
            self.buf.extend_from_slice(&to_pcm16(s).to_le_bytes());
        }
        self.output.write_all(&self.buf)?;
        self.data_bytes = total;
        Ok(())
    }

    pub fn finish(mut self, sample_rate: u32) -> Result<W, AudioError> {
        self.output.seek(SeekFrom::Start(0))?;
        self.output
            .write_all(&header(sample_rate, self.data_bytes))?;
        self.output.seek(SeekFrom::End(0))?;
        self.output.flush()?;
        Ok(self.output)
    }
}

fn to_pcm16(sample: f32) -> i16 {
    if sample.is_nan() {
        return 0;
    }
    // После clamp и round значение лежит в [-32768, 32767], приведение без потерь.
    (sample.clamp(-1.0, 1.0) * 32_768.0)
        .round()
        .clamp(-32_768.0, 32_767.0) as i16
}

fn header(sample_rate: u32, data_bytes: u32) -> [u8; HEADER_BYTES as usize] {
    let mut h = [0u8; HEADER_BYTES as usize];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(HEADER_BYTES - 8 + data_bytes).to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes());
    h[22..24].copy_from_slice(&1u16.to_le_bytes());
    h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    h[28..32].copy_from_slice(&(sample_rate * 2).to_le_bytes());
    h[32..34].copy_from_slice(&2u16.to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data_bytes.to_le_bytes());
    h
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::test_support::read_all;

    fn written(samples: &[f32]) -> Vec<u8> {
        let mut w = WavWriter::new(Cursor::new(Vec::new()), 16_000).unwrap();
        w.write(samples).unwrap();
        w.finish(16_000).unwrap().into_inner()
    }

    #[test]
    fn header_bytes_match_the_standard_layout() {
        let bytes = written(&[0.5, -0.5]);
        let expected: Vec<u8> = [
            b"RIFF".as_slice(),
            &40u32.to_le_bytes(),
            b"WAVEfmt ",
            &16u32.to_le_bytes(),
            &1u16.to_le_bytes(),
            &1u16.to_le_bytes(),
            &16_000u32.to_le_bytes(),
            &32_000u32.to_le_bytes(),
            &2u16.to_le_bytes(),
            &16u16.to_le_bytes(),
            b"data",
            &4u32.to_le_bytes(),
            &16_384i16.to_le_bytes(),
            &(-16_384i16).to_le_bytes(),
        ]
        .concat();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn round_trip_is_exact_for_pcm16_values() {
        let samples: Vec<f32> = [-32_768i16, -1, 0, 1, 12_345, 32_767]
            .iter()
            .map(|&v| f32::from(v) / 32_768.0)
            .collect();
        let (format, back) = read_all(&written(&samples)).unwrap();
        assert_eq!((format.sample_rate, format.channels), (16_000, 1));
        assert_eq!(back, samples);
    }

    #[test]
    fn out_of_range_and_nan_are_clamped() {
        let (_, back) = read_all(&written(&[2.0, -2.0, f32::NAN, f32::INFINITY])).unwrap();
        assert_eq!(back, [32_767.0 / 32_768.0, -1.0, 0.0, 32_767.0 / 32_768.0]);
    }

    #[test]
    fn several_writes_append() {
        let mut w = WavWriter::new(Cursor::new(Vec::new()), 16_000).unwrap();
        w.write(&[0.5]).unwrap();
        w.write(&[]).unwrap();
        w.write(&[-0.5]).unwrap();
        let (_, back) = read_all(&w.finish(16_000).unwrap().into_inner()).unwrap();
        assert_eq!(back, [0.5, -0.5]);
    }
}
