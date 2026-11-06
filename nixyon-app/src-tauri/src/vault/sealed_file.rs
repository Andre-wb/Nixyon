//! Файлы на диске: зашифрованные узнаются по сигнатуре, открытые (от прежних версий)
//! читаются как есть. Пишется всегда зашифрованное, через `.part` и переименование.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::data_key::{DataKey, invalid};
use super::format::{FRAME_LEN, HEADER_LEN, MAGIC, check_header, chunk_aad};
use super::sealed_reader::SealedReader;
use super::sealed_writer::SealedWriter;

pub fn is_sealed(path: &Path) -> std::io::Result<bool> {
    let mut start = [0u8; 8];
    let mut file = File::open(path)?;
    let mut got = 0;
    while got < start.len() {
        match file.read(&mut start[got..])? {
            0 => return Ok(false),
            n => got += n,
        }
    }
    Ok(&start == MAGIC)
}

/// Содержимое файла по порядку, расшифрованное, если файл зашифрован.
pub fn open_read(path: &Path, key: &Arc<DataKey>) -> std::io::Result<Box<dyn Read + Send>> {
    let sealed = is_sealed(path)?;
    let file = BufReader::new(File::open(path)?);
    Ok(if sealed {
        Box::new(SealedReader::new(Arc::clone(key), file)?)
    } else {
        Box::new(file)
    })
}

pub fn read_bytes(path: &Path, key: &Arc<DataKey>) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    open_read(path, key)?.read_to_end(&mut out)?;
    Ok(out)
}

/// Новое содержимое заменяет прежнее целиком или не заменяет вовсе.
pub fn write_atomic(path: &Path, key: &Arc<DataKey>, data: &[u8]) -> std::io::Result<()> {
    replace_with(path, key, |w| w.write_all(data))
}

/// Открытый файл становится зашифрованным на том же месте.
pub fn seal_in_place(path: &Path, key: &Arc<DataKey>) -> std::io::Result<()> {
    let mut plain = BufReader::new(File::open(path)?);
    replace_with(path, key, |w| std::io::copy(&mut plain, w).map(|_| ()))
}

fn replace_with(
    path: &Path,
    key: &Arc<DataKey>,
    fill: impl FnOnce(&mut SealedWriter<BufWriter<File>>) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let part = part_path(path);
    let result = (|| {
        let mut writer = SealedWriter::new(Arc::clone(key), BufWriter::new(File::create(&part)?))?;
        fill(&mut writer)?;
        let file = writer.finish()?.into_inner().map_err(|e| e.into_error())?;
        file.sync_all()?;
        std::fs::rename(&part, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn part_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

/// Открывается ли первый кусок этим ключом: так при запуске видно файлы, зашифрованные
/// потерянным ключом, без чтения всей записи.
pub fn first_chunk_opens(path: &Path, key: &DataKey) -> std::io::Result<bool> {
    let len = std::fs::metadata(path)?.len();
    let mut file = File::open(path)?;
    let mut header = [0u8; HEADER_LEN];
    if file.read_exact(&mut header).is_err() || !check_header(&header) {
        return Ok(false);
    }
    let mut frame = Vec::with_capacity(FRAME_LEN);
    (&mut file).take(FRAME_LEN as u64).read_to_end(&mut frame)?;
    let last = len <= (HEADER_LEN + FRAME_LEN) as u64;
    match key.open(&frame, &chunk_aad(&header, 0, last)) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == invalid().kind() => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::super::data_key::test_key;
    use super::super::format::CHUNK;
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nixyon-sealed-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn written_files_are_sealed_and_read_back_without_part_leftovers() {
        let d = dir("write");
        let key = Arc::new(test_key(1));
        let path = d.join("a.json");
        write_atomic(&path, &key, b"{\"x\":1}").unwrap();
        assert!(is_sealed(&path).unwrap());
        assert!(
            !std::fs::read(&path)
                .unwrap()
                .windows(5)
                .any(|w| w == b"\"x\":1")
        );
        assert_eq!(read_bytes(&path, &key).unwrap(), b"{\"x\":1}");
        assert!(!d.join("a.json.part").exists());
        assert!(first_chunk_opens(&path, &key).unwrap());
        assert!(!first_chunk_opens(&path, &test_key(2)).unwrap());
    }

    #[test]
    fn plain_files_from_older_versions_are_read_as_is_and_sealed_in_place() {
        let d = dir("migrate");
        let key = Arc::new(test_key(1));
        let path = d.join("rec.wav");
        let plain: Vec<u8> = (0..CHUNK * 2 + 7).map(|i| (i % 253) as u8).collect();
        std::fs::write(&path, &plain).unwrap();
        assert!(!is_sealed(&path).unwrap());
        assert_eq!(read_bytes(&path, &key).unwrap(), plain);
        seal_in_place(&path, &key).unwrap();
        assert!(is_sealed(&path).unwrap());
        assert_eq!(read_bytes(&path, &key).unwrap(), plain);
        assert!(first_chunk_opens(&path, &key).unwrap());
        std::fs::write(d.join("short"), b"abc").unwrap();
        assert!(!is_sealed(&d.join("short")).unwrap());
    }

    #[test]
    fn sealed_file_under_a_lost_key_does_not_read() {
        let d = dir("lost");
        let path = d.join("v.json");
        write_atomic(&path, &Arc::new(test_key(1)), b"[]").unwrap();
        assert!(read_bytes(&path, &Arc::new(test_key(2))).is_err());
    }
}
