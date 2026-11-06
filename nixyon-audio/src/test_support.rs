use std::io::Cursor;
use std::path::{Path, PathBuf};

use crate::{AudioError, WavFormat, WavReader};

pub fn read_all(bytes: &[u8]) -> Result<(WavFormat, Vec<f32>), AudioError> {
    let mut reader = WavReader::new(Cursor::new(bytes))?;
    let mut out = Vec::new();
    while reader.read_mono(5, &mut out)? > 0 {}
    Ok((reader.format(), out))
}

pub fn sine(rate: u32, freq: f32, amplitude: f32, seconds: f64) -> Vec<f32> {
    let n = (f64::from(rate) * seconds).round() as usize;
    (0..n)
        .map(|i| amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin())
        .collect()
}

pub fn zero_crossings(samples: &[f32]) -> usize {
    samples
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count()
}

/// Сборщик WAV-файлов для тестов, в том числе нестандартных и битых.
pub struct WavBytes {
    format_tag: u16,
    channels: u16,
    rate: u32,
    bits: u16,
    data: Vec<u8>,
    extensible: bool,
    extra: Option<(&'static [u8; 4], usize)>,
    unknown_size: bool,
    huge_chunk: bool,
}

impl WavBytes {
    pub fn pcm16(rate: u32, channels: u16, samples: &[i16]) -> Self {
        Self::new(
            1,
            16,
            rate,
            channels,
            samples.iter().flat_map(|s| s.to_le_bytes()).collect(),
        )
    }

    pub fn float32(rate: u32, channels: u16, samples: &[f32]) -> Self {
        Self::new(
            3,
            32,
            rate,
            channels,
            samples.iter().flat_map(|s| s.to_le_bytes()).collect(),
        )
    }

    fn new(format_tag: u16, bits: u16, rate: u32, channels: u16, data: Vec<u8>) -> Self {
        Self {
            format_tag,
            channels,
            rate,
            bits,
            data,
            extensible: false,
            extra: None,
            unknown_size: false,
            huge_chunk: false,
        }
    }

    pub fn bits(mut self, bits: u16) -> Self {
        self.bits = bits;
        self
    }

    pub fn extensible(mut self) -> Self {
        self.extensible = true;
        self
    }

    pub fn extra_chunk(mut self, id: &'static [u8; 4], size: usize) -> Self {
        self.extra = Some((id, size));
        self
    }

    pub fn unknown_data_size(mut self) -> Self {
        self.unknown_size = true;
        self
    }

    pub fn huge_chunk_before_data(mut self) -> Self {
        self.huge_chunk = true;
        self
    }

    pub fn build(self) -> Vec<u8> {
        let mut fmt = Vec::new();
        let tag = if self.extensible {
            0xFFFE
        } else {
            self.format_tag
        };
        fmt.extend_from_slice(&tag.to_le_bytes());
        fmt.extend_from_slice(&self.channels.to_le_bytes());
        fmt.extend_from_slice(&self.rate.to_le_bytes());
        let block = u32::from(self.channels) * u32::from(self.bits / 8);
        fmt.extend_from_slice(&(self.rate * block).to_le_bytes());
        fmt.extend_from_slice(&(block as u16).to_le_bytes());
        fmt.extend_from_slice(&self.bits.to_le_bytes());
        if self.extensible {
            fmt.extend_from_slice(&22u16.to_le_bytes());
            fmt.extend_from_slice(&self.bits.to_le_bytes());
            fmt.extend_from_slice(&0u32.to_le_bytes());
            fmt.extend_from_slice(&self.format_tag.to_le_bytes());
            fmt.extend_from_slice(&[0u8; 14]);
        }

        let mut body = b"WAVE".to_vec();
        push_chunk(&mut body, b"fmt ", &fmt);
        if let Some((id, size)) = self.extra {
            push_chunk(&mut body, id, &vec![7u8; size]);
        }
        if self.huge_chunk {
            body.extend_from_slice(b"JUNK");
            body.extend_from_slice(&u32::MAX.to_le_bytes());
        }
        body.extend_from_slice(b"data");
        let size = if self.unknown_size {
            u32::MAX
        } else {
            self.data.len() as u32
        };
        body.extend_from_slice(&size.to_le_bytes());
        body.extend_from_slice(&self.data);

        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }
}

fn push_chunk(out: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
}

/// Временная папка теста, удаляется при выходе из теста.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nixyon-audio-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
