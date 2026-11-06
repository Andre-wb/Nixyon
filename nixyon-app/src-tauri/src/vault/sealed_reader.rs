use std::io::Read;
use std::sync::Arc;

use super::data_key::{DataKey, invalid};
use super::format::{FRAME_LEN, HEADER_LEN, Header, check_header, chunk_aad};

/// Потоковое чтение зашифрованного файла по порядку. Следующий кусок читается заранее:
/// только так видно, что текущий последний. Обрезанный, переставленный или подменённый
/// файл даёт ошибку `InvalidData`, а не часть данных молча.
pub struct SealedReader<R: Read> {
    key: Arc<DataKey>,
    input: R,
    header: Header,
    index: u64,
    plain: Vec<u8>,
    pos: usize,
    next: Vec<u8>,
    done: bool,
}

impl<R: Read> SealedReader<R> {
    pub fn new(key: Arc<DataKey>, mut input: R) -> std::io::Result<Self> {
        let mut header = [0u8; HEADER_LEN];
        input.read_exact(&mut header).map_err(|_| invalid())?;
        if !check_header(&header) {
            return Err(invalid());
        }
        let next = read_frame(&mut input)?;
        if next.is_empty() {
            return Err(invalid());
        }
        Ok(Self {
            key,
            input,
            header,
            index: 0,
            plain: Vec::new(),
            pos: 0,
            next,
            done: false,
        })
    }

    fn fill(&mut self) -> std::io::Result<()> {
        let current = std::mem::take(&mut self.next);
        self.next = read_frame(&mut self.input)?;
        let last = self.next.is_empty();
        if !last && current.len() != FRAME_LEN {
            return Err(invalid());
        }
        let aad = chunk_aad(&self.header, self.index, last);
        self.plain = self.key.open(&current, &aad)?;
        self.pos = 0;
        self.index += 1;
        self.done = last;
        Ok(())
    }
}

impl<R: Read> Read for SealedReader<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.pos == self.plain.len() {
            if self.done {
                return Ok(0);
            }
            self.fill()?;
        }
        let n = out.len().min(self.plain.len() - self.pos);
        out[..n].copy_from_slice(&self.plain[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

// До FRAME_LEN байт; меньше только в конце файла.
fn read_frame(input: &mut impl Read) -> std::io::Result<Vec<u8>> {
    let mut frame = Vec::with_capacity(FRAME_LEN);
    input.take(FRAME_LEN as u64).read_to_end(&mut frame)?;
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::sync::Arc;

    use super::super::data_key::test_key;
    use super::super::format::{CHUNK, FRAME_LEN, HEADER_LEN};
    use super::super::sealed_writer::SealedWriter;
    use super::SealedReader;

    fn seal(data: &[u8], key: u8) -> Vec<u8> {
        let mut w = SealedWriter::new(Arc::new(test_key(key)), Vec::new()).unwrap();
        // Мелкими порциями: куски должны набираться через границы записей.
        for part in data.chunks(1000) {
            w.write_all(part).unwrap();
        }
        w.finish().unwrap()
    }

    fn open(bytes: &[u8], key: u8) -> std::io::Result<Vec<u8>> {
        let mut out = Vec::new();
        SealedReader::new(Arc::new(test_key(key)), bytes)?.read_to_end(&mut out)?;
        Ok(out)
    }

    fn data(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 31 % 251) as u8).collect()
    }

    #[test]
    fn any_length_round_trips_including_empty_and_exact_chunks() {
        for len in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 2 * CHUNK, 2 * CHUNK + 5] {
            let plain = data(len);
            let sealed = seal(&plain, 1);
            let chunks = len.div_ceil(CHUNK).max(1);
            assert_eq!(
                sealed.len(),
                HEADER_LEN + len + chunks * (FRAME_LEN - CHUNK)
            );
            assert_eq!(open(&sealed, 1).unwrap(), plain, "длина {len}");
        }
    }

    #[test]
    fn chunk_offsets_are_fixed_so_any_chunk_can_be_found_directly() {
        let sealed = seal(&data(2 * CHUNK + 5), 1);
        assert_eq!(sealed.len() - HEADER_LEN - 2 * FRAME_LEN, 12 + 5 + 16);
    }

    #[test]
    fn wrong_key_truncation_reordering_and_foreign_chunks_are_rejected_cwe_345() {
        let plain = data(3 * CHUNK);
        let sealed = seal(&plain, 1);
        assert!(open(&sealed, 2).is_err());
        // Обрезка по границе куска: новый «последний» кусок был зашифрован как не последний.
        assert!(open(&sealed[..HEADER_LEN + 2 * FRAME_LEN], 1).is_err());
        assert!(open(&sealed[..sealed.len() - 1], 1).is_err());
        let mut swapped = sealed.clone();
        let (a, b) = (HEADER_LEN, HEADER_LEN + FRAME_LEN);
        let first = sealed[a..b].to_vec();
        swapped.copy_within(b..b + FRAME_LEN, a);
        swapped[b..b + FRAME_LEN].copy_from_slice(&first);
        assert!(open(&swapped, 1).is_err());
        // Кусок того же номера из другого файла с тем же ключом.
        let other = seal(&plain, 1);
        let mut mixed = sealed.clone();
        mixed[a..b].copy_from_slice(&other[a..b]);
        assert!(open(&mixed, 1).is_err());
        assert!(open(&sealed[..HEADER_LEN], 1).is_err());
        assert!(open(b"RIFF....WAVE", 1).is_err());
    }
}
