#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("файл расшифровки не читается: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("неизвестная версия формата расшифровки: {found}, поддерживается {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
}
