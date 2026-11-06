#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleEncoding {
    Pcm16,
    Float32,
}

impl SampleEncoding {
    pub fn bytes(self) -> usize {
        match self {
            SampleEncoding::Pcm16 => 2,
            SampleEncoding::Float32 => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub encoding: SampleEncoding,
}

impl WavFormat {
    pub fn frame_bytes(&self) -> usize {
        usize::from(self.channels) * self.encoding.bytes()
    }
}
