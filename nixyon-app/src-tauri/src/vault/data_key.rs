use aes_gcm::aead::{Aead, Generate, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};

use super::format::{NONCE_LEN, TAG_LEN};

/// Ключ данных AES-256-GCM. Сам ключ приходит из Kotlin, где он хранится обёрнутым
/// ключом Android Keystore; в лог и в ошибки не попадает.
pub struct DataKey(Aes256Gcm);

impl DataKey {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let key = Key::<Aes256Gcm>::try_from(bytes).ok()?;
        Some(Self(Aes256Gcm::new(&key)))
    }

    /// nonce, шифртекст и тег одним куском.
    pub fn seal(&self, plain: &[u8], aad: &[u8]) -> std::io::Result<Vec<u8>> {
        let nonce = Nonce::try_generate().map_err(std::io::Error::other)?;
        let sealed = self
            .0
            .encrypt(&nonce, Payload { msg: plain, aad })
            .map_err(|_| std::io::Error::other("шифрование не удалось"))?;
        let mut out = Vec::with_capacity(NONCE_LEN + sealed.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        Ok(out)
    }

    /// Чужой ключ, подмена или повреждение дают ошибку `InvalidData`.
    pub fn open(&self, frame: &[u8], aad: &[u8]) -> std::io::Result<Vec<u8>> {
        if frame.len() < NONCE_LEN + TAG_LEN {
            return Err(invalid());
        }
        let (nonce, sealed) = frame.split_at(NONCE_LEN);
        let nonce = Nonce::try_from(nonce).map_err(|_| invalid())?;
        self.0
            .decrypt(&nonce, Payload { msg: sealed, aad })
            .map_err(|_| invalid())
    }
}

pub fn invalid() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, "файл не расшифровывается")
}

#[cfg(test)]
pub fn test_key(byte: u8) -> DataKey {
    DataKey::from_bytes(&[byte; 32]).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_bytes_open_only_with_the_same_key_and_associated_data() {
        let key = test_key(1);
        let frame = key.seal(b"privet", b"aad").unwrap();
        assert_eq!(frame.len(), NONCE_LEN + 6 + TAG_LEN);
        assert_eq!(key.open(&frame, b"aad").unwrap(), b"privet");
        assert!(key.open(&frame, b"other").is_err());
        assert!(test_key(2).open(&frame, b"aad").is_err());
        let mut broken = frame.clone();
        broken[NONCE_LEN] ^= 1;
        assert!(key.open(&broken, b"aad").is_err());
        assert!(key.open(&frame[..10], b"aad").is_err());
    }

    #[test]
    fn every_seal_uses_a_fresh_nonce() {
        let key = test_key(1);
        let a = key.seal(b"x", b"").unwrap();
        let b = key.seal(b"x", b"").unwrap();
        assert_ne!(a[..NONCE_LEN], b[..NONCE_LEN]);
    }

    #[test]
    fn key_must_be_32_bytes() {
        assert!(DataKey::from_bytes(&[0; 16]).is_none());
        assert!(DataKey::from_bytes(&[0; 33]).is_none());
    }
}
