//! Формат зашифрованного файла, общий с Kotlin-плагином (`SealedFormat.kt`).
//! Заголовок: сигнатура, размер куска открытого текста u32 LE, четыре нуля, случайный
//! id файла. Дальше куски: nonce, шифртекст, тег. Все куски, кроме последнего, полные.
//! Связанные данные куска: заголовок, номер куска u64 LE и признак последнего; так
//! куски нельзя переставить, обрезать файл или перенести кусок в другой файл.

pub const MAGIC: &[u8; 8] = b"NXSEAL01";
pub const CHUNK: usize = 65_536;
pub const HEADER_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;
pub const FRAME_LEN: usize = NONCE_LEN + CHUNK + TAG_LEN;
const FILE_ID_LEN: usize = 16;

pub type Header = [u8; HEADER_LEN];

pub fn header(file_id: [u8; FILE_ID_LEN]) -> Header {
    let mut h = [0u8; HEADER_LEN];
    h[..8].copy_from_slice(MAGIC);
    h[8..12].copy_from_slice(&(CHUNK as u32).to_le_bytes());
    h[16..].copy_from_slice(&file_id);
    h
}

/// Заголовок другого формата или с другим размером куска не принимается.
pub fn check_header(h: &Header) -> bool {
    h[..8] == MAGIC[..] && h[8..12] == (CHUNK as u32).to_le_bytes() && h[12..16] == [0; 4]
}

pub fn chunk_aad(header: &Header, index: u64, last: bool) -> [u8; HEADER_LEN + 9] {
    let mut aad = [0u8; HEADER_LEN + 9];
    aad[..HEADER_LEN].copy_from_slice(header);
    aad[HEADER_LEN..HEADER_LEN + 8].copy_from_slice(&index.to_le_bytes());
    aad[HEADER_LEN + 8] = u8::from(last);
    aad
}

pub fn new_file_id() -> std::io::Result<[u8; FILE_ID_LEN]> {
    use aes_gcm::aead::Generate;
    <[u8; FILE_ID_LEN]>::try_generate().map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_carries_magic_chunk_size_and_file_id() {
        let h = header([7; 16]);
        assert_eq!(&h[..8], b"NXSEAL01");
        assert_eq!(&h[8..12], &[0, 0, 1, 0]);
        assert_eq!(&h[12..16], &[0; 4]);
        assert_eq!(&h[16..], &[7; 16]);
        assert!(check_header(&h));
        let mut other = h;
        other[9] = 2;
        assert!(!check_header(&other));
    }

    #[test]
    fn chunk_aad_binds_header_index_and_last_flag() {
        let h = header([1; 16]);
        let a = chunk_aad(&h, 258, true);
        assert_eq!(&a[..32], &h);
        assert_eq!(&a[32..40], &[2, 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(a[40], 1);
        assert_eq!(chunk_aad(&h, 258, false)[40], 0);
    }
}
