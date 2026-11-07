use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use nixyon_audio::WavReader;
use nixyon_core::{ModelOverlaps, OverlapGrid, PowersetWindow};

use crate::OrtError;
use crate::pyannote_powerset::{
    PyannotePowerset, SAMPLE_RATE, WINDOW_SAMPLES, WINDOW_SHIFT_SAMPLES,
};

/// Сколько кадров звука читается из файла за раз.
const READ_FRAMES: usize = 16_000;

/// Перекрытия по модели сегментации в два прохода по WAV 16 кГц, без записи целиком в
/// памяти. Первый проход гонит модель по всей записи окнами 10 с и копит только среднюю
/// вероятность перекрытия по кадрам; второй заново считает окна, которые заходят в
/// найденные перекрытия, и только их отдаёт. Окна те же, что при одном проходе: последнее
/// дополняется тишиной, кадры после конца записи отбрасываются.
/// `keep_going` получает пройденные секунды первого прохода; `false` отменяет обход, тогда
/// итог `None`.
pub fn scan_overlaps(
    model: &PyannotePowerset,
    wav: &Path,
    threshold: f32,
    min_sec: f64,
    keep_going: &mut dyn FnMut(f64) -> bool,
) -> Result<Option<ModelOverlaps>, OrtError> {
    let mut grid = OverlapGrid::new();
    let mut starts: Vec<usize> = Vec::new();
    let mut stream = WindowStream::open(wav)?;
    let mut start = 0;
    while let Some(piece) = stream.window_at(start)? {
        let window = model.window(piece.samples, start, piece.length)?;
        grid.add(&window);
        starts.push(start);
        if !keep_going((start + piece.length) as f64 / SAMPLE_RATE) {
            return Ok(None);
        }
        if piece.last {
            break;
        }
        start += WINDOW_SHIFT_SAMPLES;
    }
    let spans = grid.spans(threshold, min_sec);
    let mut windows: Vec<PowersetWindow> = Vec::new();
    let mut stream = WindowStream::open(wav)?;
    for start in starts.into_iter().filter(|&s| touches(s, &spans)) {
        let Some(piece) = stream.window_at(start)? else {
            break;
        };
        windows.push(model.window(piece.samples, start, piece.length)?);
    }
    Ok(Some(ModelOverlaps { spans, windows }))
}

// Окно, которое начинается с отсчёта `start`, заходит хотя бы в одно перекрытие.
fn touches(start: usize, spans: &[(f64, f64)]) -> bool {
    let from = start as f64 / SAMPLE_RATE;
    let to = (start + WINDOW_SAMPLES) as f64 / SAMPLE_RATE;
    spans.iter().any(|&(a, b)| a < to && from < b)
}

// Звук окна, дополненный тишиной до длины окна; `length`: сколько в нём настоящих
// отсчётов; `last`: окно дошло до конца записи.
struct Piece {
    samples: Vec<f32>,
    length: usize,
    last: bool,
}

// Читает WAV потоком и держит в памяти только звук от начала текущего окна. Окна идут
// с возрастающим началом.
struct WindowStream {
    reader: WavReader<BufReader<File>>,
    buffer: Vec<f32>,
    buffer_start: usize,
    ended: bool,
}

impl WindowStream {
    fn open(wav: &Path) -> Result<Self, OrtError> {
        let file = File::open(wav).map_err(|source| OrtError::Read {
            path: wav.to_path_buf(),
            source,
        })?;
        let reader = WavReader::new(BufReader::new(file)).map_err(audio)?;
        let rate = reader.format().sample_rate;
        if f64::from(rate) != SAMPLE_RATE {
            return Err(OrtError::Audio(format!("частота {rate} Гц, ждали 16000")));
        }
        Ok(Self {
            reader,
            buffer: Vec::new(),
            buffer_start: 0,
            ended: false,
        })
    }

    // None: с этого места звука нет.
    fn window_at(&mut self, start: usize) -> Result<Option<Piece>, OrtError> {
        // Звук до начала окна больше не нужен, а если окно дальше буфера, звук до него
        // читается и отбрасывается.
        while self.buffer_start + self.buffer.len() < start && !self.ended {
            self.buffer_start += self.buffer.len();
            self.buffer.clear();
            self.read_more()?;
        }
        let skip = start
            .saturating_sub(self.buffer_start)
            .min(self.buffer.len());
        self.buffer.drain(..skip);
        self.buffer_start += skip;
        // Отсчёт сверх окна показывает, есть ли звук дальше него.
        while self.buffer.len() <= WINDOW_SAMPLES && !self.ended {
            self.read_more()?;
        }
        if self.buffer_start != start || self.buffer.is_empty() {
            return Ok(None);
        }
        let length = self.buffer.len().min(WINDOW_SAMPLES);
        let mut samples = self.buffer[..length].to_vec();
        samples.resize(WINDOW_SAMPLES, 0.0);
        Ok(Some(Piece {
            samples,
            length,
            last: self.buffer.len() <= WINDOW_SAMPLES,
        }))
    }

    fn read_more(&mut self) -> Result<(), OrtError> {
        let read = self
            .reader
            .read_mono(READ_FRAMES, &mut self.buffer)
            .map_err(audio)?;
        if read == 0 {
            self.ended = true;
        }
        Ok(())
    }
}

fn audio(e: nixyon_audio::AudioError) -> OrtError {
    OrtError::Audio(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_touches_only_overlaps_inside_its_ten_seconds() {
        let spans = [(12.0, 13.0)];
        assert!(touches(16_000 * 3, &spans));
        assert!(!touches(16_000 * 13, &spans));
        assert!(!touches(0, &spans));
    }
}
