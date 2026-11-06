use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::AudioError;

const OUTPUT_RATE: u32 = 16_000;
const CHUNK_FRAMES: usize = 1_024;

/// Потоковый перевод моно-звука в 16 кГц. Результат по длине и положению совпадает
/// с исходником: задержка фильтра на старте отрезается, хвост дожимается в `finish`.
pub struct Resampler16k {
    inner: Option<Inner>,
}

struct Inner {
    resampler: Fft<f32>,
    pending: Vec<f32>,
    out_buf: Vec<f32>,
    delay_left: usize,
    input_total: u64,
    output_total: u64,
    input_rate: u32,
}

impl Resampler16k {
    pub fn new(input_rate: u32) -> Result<Self, AudioError> {
        if input_rate == 0 {
            return Err(AudioError::BadFormat("частота дискретизации"));
        }
        if input_rate == OUTPUT_RATE {
            return Ok(Self { inner: None });
        }
        let resampler = Fft::<f32>::new(
            input_rate as usize,
            OUTPUT_RATE as usize,
            CHUNK_FRAMES,
            1,
            FixedSync::Input,
        )
        .map_err(|e| AudioError::Resample(e.to_string()))?;
        Ok(Self {
            inner: Some(Inner {
                delay_left: resampler.output_delay(),
                out_buf: vec![0.0; resampler.output_frames_max()],
                resampler,
                pending: Vec::with_capacity(CHUNK_FRAMES * 2),
                input_total: 0,
                output_total: 0,
                input_rate,
            }),
        })
    }

    pub fn push(&mut self, samples: &[f32], out: &mut Vec<f32>) -> Result<(), AudioError> {
        let Some(inner) = self.inner.as_mut() else {
            out.extend_from_slice(samples);
            return Ok(());
        };
        inner.input_total += samples.len() as u64;
        inner.pending.extend_from_slice(samples);
        while inner.pending.len() >= inner.resampler.input_frames_next() {
            let need = inner.resampler.input_frames_next();
            let used = inner.process(need, None, out)?;
            inner.pending.drain(..used);
        }
        Ok(())
    }

    pub fn finish(self, out: &mut Vec<f32>) -> Result<(), AudioError> {
        let Some(mut inner) = self.inner else {
            return Ok(());
        };
        let expected = inner.expected_output();
        // Остаток входа добивается нулями, потом нули подаются, пока не выйдет
        // весь хвост с учётом задержки фильтра.
        let mut partial = inner.pending.len();
        while inner.output_total < expected {
            let need = inner.resampler.input_frames_next();
            inner.pending.resize(need, 0.0);
            let produced_before = inner.output_total;
            inner.process(need, Some(partial), out)?;
            inner.pending.clear();
            partial = 0;
            if inner.output_total == produced_before && inner.delay_left == 0 {
                return Err(AudioError::Resample(
                    "ресемплер перестал выдавать данные".into(),
                ));
            }
        }
        Ok(())
    }
}

impl Inner {
    fn expected_output(&self) -> u64 {
        let out = u64::from(OUTPUT_RATE);
        let input = u64::from(self.input_rate);
        (self.input_total * out).div_ceil(input)
    }

    /// Прогоняет один кусок из начала `pending`. Возвращает, сколько входа ушло.
    fn process(
        &mut self,
        need: usize,
        partial: Option<usize>,
        out: &mut Vec<f32>,
    ) -> Result<usize, AudioError> {
        let input = InterleavedSlice::new(&self.pending[..need], 1, need)
            .map_err(|e| AudioError::Resample(e.to_string()))?;
        let frames_out = self.out_buf.len();
        let mut output = InterleavedSlice::new_mut(&mut self.out_buf, 1, frames_out)
            .map_err(|e| AudioError::Resample(e.to_string()))?;
        let indexing = partial.map(|n| Indexing::new().partial_len(n));
        let (used, produced) = self
            .resampler
            .process_into_buffer(&input, &mut output, indexing.as_ref())
            .map_err(|e| AudioError::Resample(e.to_string()))?;

        let skip = self.delay_left.min(produced);
        self.delay_left -= skip;
        let expected = self.expected_output();
        let room =
            usize::try_from(expected.saturating_sub(self.output_total)).unwrap_or(usize::MAX);
        let take = (produced - skip).min(room);
        out.extend_from_slice(&self.out_buf[skip..skip + take]);
        self.output_total += take as u64;
        Ok(used)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{sine, zero_crossings};

    fn resample_in_chunks(rate: u32, input: &[f32], chunk: usize) -> Vec<f32> {
        let mut r = Resampler16k::new(rate).unwrap();
        let mut out = Vec::new();
        for c in input.chunks(chunk) {
            r.push(c, &mut out).unwrap();
        }
        r.finish(&mut out).unwrap();
        out
    }

    #[test]
    fn length_is_exactly_input_length_times_ratio_rounded_up() {
        for (rate, frames) in [
            (48_000, 48_000),
            (44_100, 44_100),
            (44_100, 1),
            (8_000, 12_345),
            (48_000, 0),
        ] {
            let input = vec![0.1; frames];
            let out = resample_in_chunks(rate, &input, 700);
            let expected = (frames as u64 * 16_000).div_ceil(u64::from(rate));
            assert_eq!(out.len() as u64, expected, "{rate} Гц, {frames} кадров");
        }
    }

    #[test]
    fn sine_keeps_its_frequency_and_amplitude() {
        for rate in [48_000, 44_100, 22_050] {
            let out = resample_in_chunks(rate, &sine(rate, 440.0, 0.5, 1.0), 4_096);
            // 440 Гц за секунду дают 880 переходов через ноль.
            let crossings = zero_crossings(&out[800..15_200]);
            let expected = 880.0 * (15_200.0 - 800.0) / 16_000.0;
            assert!(
                (crossings as f64 - expected).abs() <= 2.0,
                "{rate}: {crossings}"
            );
            let peak = out[800..15_200].iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!((peak - 0.5).abs() < 0.01, "{rate}: пик {peak}");
        }
    }

    #[test]
    fn signal_is_not_shifted_in_time() {
        // Щелчок посередине секунды должен остаться посередине.
        let mut input = vec![0.0; 48_000];
        input[24_000] = 1.0;
        let out = resample_in_chunks(48_000, &input, 1_000);
        let peak_at = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap()
            .0;
        assert!((peak_at as i64 - 8_000).abs() <= 1, "пик на {peak_at}");
    }

    #[test]
    fn chunk_size_does_not_change_the_result() {
        let input = sine(44_100, 300.0, 0.4, 0.5);
        let a = resample_in_chunks(44_100, &input, 1);
        let b = resample_in_chunks(44_100, &input, 100_000);
        assert_eq!(a, b);
    }

    #[test]
    fn sixteen_khz_input_passes_through_unchanged() {
        let input = sine(16_000, 440.0, 0.5, 0.3);
        assert_eq!(resample_in_chunks(16_000, &input, 333), input);
    }

    #[test]
    fn zero_rate_is_rejected() {
        assert!(matches!(
            Resampler16k::new(0),
            Err(AudioError::BadFormat(_))
        ));
    }
}
