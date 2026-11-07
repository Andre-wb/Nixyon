use std::sync::{Arc, Mutex};

use nixyon_core::{Pool, SpeechLanguage};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig};

use crate::{Decoding, ModelManifest, SherpaError, config};

type Recognizers = Arc<Pool<OfflineRecognizer>>;

/// Распознаватели одного языка за раз: GigaAM и Parakeet вместе занимают слишком много
/// памяти. Запись на другом языке отпускает загруженные и грузит свои. Кто ещё
/// распознаёт прежним языком, держит их через Arc, и они освобождаются после него.
pub struct LanguageRecognizers {
    manifest: ModelManifest,
    decoding: Decoding,
    threads: i32,
    jobs: usize,
    loaded: Mutex<Option<(SpeechLanguage, Recognizers)>>,
}

impl LanguageRecognizers {
    /// Сразу загружает распознаватели языка `first`: ошибка в модели видна при загрузке.
    pub fn load(
        manifest: &ModelManifest,
        first: SpeechLanguage,
        decoding: Decoding,
        threads: i32,
        jobs: usize,
    ) -> Result<Self, SherpaError> {
        let set = Self {
            manifest: manifest.clone(),
            decoding,
            threads,
            jobs: jobs.max(1),
            loaded: Mutex::new(None),
        };
        set.of(first)?;
        Ok(set)
    }

    /// Распознаватели языка `language`; другой язык выгружается.
    pub fn of(&self, language: SpeechLanguage) -> Result<Recognizers, SherpaError> {
        let mut loaded = self.loaded.lock().map_err(|_| SherpaError::Poisoned)?;
        if let Some((current, pool)) = loaded.as_ref()
            && *current == language
        {
            return Ok(Arc::clone(pool));
        }
        // Сначала отпустить прежний язык, потом грузить новый: две модели сразу не нужны.
        *loaded = None;
        let config = config::recognizer(&self.manifest, language, self.decoding, self.threads)?;
        let pool = Arc::new(self.create(&config)?);
        *loaded = Some((language, Arc::clone(&pool)));
        Ok(pool)
    }

    /// Сколько кусков речи распознаётся параллельно.
    pub fn jobs(&self) -> usize {
        self.jobs
    }

    /// Способ поиска, заданный при загрузке; у английского свой (`Decoding::for_language`).
    pub fn decoding(&self) -> Decoding {
        self.decoding
    }

    fn create(
        &self,
        config: &OfflineRecognizerConfig,
    ) -> Result<Pool<OfflineRecognizer>, SherpaError> {
        let one =
            || OfflineRecognizer::create(config).ok_or(SherpaError::CreateFailed("распознаватель"));
        let first = one()?;
        let rest = (1..self.jobs)
            .map(|_| one())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Pool::with_first(first, rest))
    }
}
