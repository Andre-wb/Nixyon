//! Словари интерфейса ru/en: проверка и подписи экспорта на языке интерфейса.
//! Словари лежат в `nixyon-app/ui/locales` и встраиваются при сборке.

mod check;
mod dictionary;
mod entries;
mod export_labels;
mod js_module;
mod lang;
mod locale_error;
mod placeholders;

pub use check::{check, check_embedded};
pub use dictionary::Dictionary;
pub use export_labels::export_labels;
pub use js_module::{GenerateError, generate, js_identifier, used_keys};
pub use lang::Lang;
pub use locale_error::LocaleError;
