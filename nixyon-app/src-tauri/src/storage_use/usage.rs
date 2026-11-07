use serde::Serialize;

/// Сколько байт на диске занимает каждая часть данных приложения.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct StorageUsage {
    /// Звук встреч и ещё не расшифрованная запись.
    pub recordings: u64,
    /// Всё остальное в папках встреч и индекс архива.
    pub transcripts: u64,
    /// Скачанные модели; встроенные в APK не считаются.
    pub models: u64,
    pub voices: u64,
    pub temporary: u64,
}
