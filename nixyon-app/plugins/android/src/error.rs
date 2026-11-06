use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
    #[error("распознавание речи на этой платформе не поддерживается")]
    Unsupported,
}

impl Error {
    /// Код отказа из Kotlin (например, "cancelled" или "models_not_found"), если он был.
    pub fn rejected_code(&self) -> Option<&str> {
        match self {
            #[cfg(mobile)]
            Error::PluginInvoke(tauri::plugin::mobile::PluginInvokeError::InvokeRejected(r)) => {
                r.code.as_deref()
            }
            _ => None,
        }
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
