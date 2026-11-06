use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;

/// Читает `length` байт файла начиная с `offset`. Модель лежит отдельным файлом
/// (смещение 0) или несжатой внутри APK, тогда смещение и длину даёт Android.
pub fn read_model_slice(file: &File, offset: u64, length: u64) -> io::Result<Vec<u8>> {
    if length == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "длина модели 0",
        ));
    }
    let length = usize::try_from(length).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "модель не помещается в память процесса",
        )
    })?;
    let mut bytes = vec![0u8; length];
    file.read_exact_at(&mut bytes, offset)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::PathBuf;

    use super::*;

    struct TempFile(PathBuf);

    impl TempFile {
        fn with(name: &str, bytes: &[u8]) -> Self {
            let path = std::env::temp_dir()
                .join(format!("nixyon-model-slice-{}-{name}", std::process::id()));
            File::create(&path).unwrap().write_all(bytes).unwrap();
            Self(path)
        }

        fn open(&self) -> File {
            File::open(&self.0).unwrap()
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn slice_inside_a_bigger_file_is_read_from_its_offset() {
        let file = TempFile::with("inside", b"apk-header|model-bytes|tail");
        assert_eq!(
            read_model_slice(&file.open(), 11, 11).unwrap(),
            b"model-bytes"
        );
    }

    #[test]
    fn whole_file_is_read_from_offset_zero() {
        let file = TempFile::with("whole", b"model");
        assert_eq!(read_model_slice(&file.open(), 0, 5).unwrap(), b"model");
    }

    #[test]
    fn slice_past_the_end_of_the_file_is_an_error() {
        let file = TempFile::with("past", b"short");
        let error = read_model_slice(&file.open(), 2, 10).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn empty_slice_is_an_error() {
        let file = TempFile::with("empty", b"model");
        let error = read_model_slice(&file.open(), 0, 0).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
