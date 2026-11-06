use std::collections::BTreeMap;

use crate::entries::parse;
use crate::lang::Lang;
use crate::locale_error::LocaleError;

/// Словарь одного языка из сборки. Повторы и порядок ловит `check` в тестах; здесь
/// при повторе берётся последнее значение, как в JSON.
pub struct Dictionary {
    lang: Lang,
    texts: BTreeMap<String, String>,
}

impl Dictionary {
    pub fn load(lang: Lang) -> Result<Self, LocaleError> {
        let entries = parse(lang.source()).map_err(|reason| LocaleError::NotJson {
            lang: lang.code(),
            reason,
        })?;
        Ok(Self {
            lang,
            texts: entries.into_iter().map(|e| (e.key, e.value)).collect(),
        })
    }

    pub fn get(&self, key: &str) -> Result<&str, LocaleError> {
        self.texts
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| LocaleError::NoKey {
                lang: self.lang.code(),
                key: key.into(),
            })
    }
}
