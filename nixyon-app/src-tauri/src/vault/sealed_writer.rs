use std::io::Write;
use std::sync::Arc;

use super::data_key::DataKey;
use super::format::{CHUNK, Header, chunk_aad, header, new_file_id};

/// Потоковое шифрование: заголовок сразу, затем куски по мере набора. Полный кусок
/// уходит только когда пришли следующие байты, иначе неизвестно, последний ли он.
/// `finish` обязателен: без него нет последнего куска и файл не прочитается.
pub struct SealedWriter<W: Write> {
    key: Arc<DataKey>,
    out: W,
    header: Header,
    index: u64,
    buf: Vec<u8>,
}

impl<W: Write> SealedWriter<W> {
    pub fn new(key: Arc<DataKey>, mut out: W) -> std::io::Result<Self> {
        let header = header(new_file_id()?);
        out.write_all(&header)?;
        Ok(Self {
            key,
            out,
            header,
            index: 0,
            buf: Vec::with_capacity(CHUNK),
        })
    }

    pub fn finish(mut self) -> std::io::Result<W> {
        self.emit(true)?;
        self.out.flush()?;
        Ok(self.out)
    }

    fn emit(&mut self, last: bool) -> std::io::Result<()> {
        let aad = chunk_aad(&self.header, self.index, last);
        let frame = self.key.seal(&self.buf, &aad)?;
        self.out.write_all(&frame)?;
        self.buf.clear();
        self.index += 1;
        Ok(())
    }
}

impl<W: Write> Write for SealedWriter<W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut rest = data;
        while !rest.is_empty() {
            if self.buf.len() == CHUNK {
                self.emit(false)?;
            }
            let take = rest.len().min(CHUNK - self.buf.len());
            self.buf.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}
