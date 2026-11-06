use crate::entries::{Entry, parse};
use crate::lang::Lang;
use crate::locale_error::LocaleError;
use crate::placeholders::placeholders;

/// Проверка словарей по русскому эталону: JSON-объект строк, ключи без повторов и по
/// алфавиту, значения не пустые, у английского тот же набор ключей и те же подстановки.
/// Пустой список: словари в порядке.
pub fn check(ru: &str, en: &str) -> Vec<LocaleError> {
    let mut errors = Vec::new();
    let ru_entries = own(Lang::Ru, ru, &mut errors);
    let en_entries = own(Lang::En, en, &mut errors);
    if let (Some(ru), Some(en)) = (ru_entries, en_entries) {
        against_reference(Lang::En, &ru, &en, &mut errors);
    }
    errors
}

/// Проверка словарей, встроенных в сборку.
pub fn check_embedded() -> Vec<LocaleError> {
    check(Lang::Ru.source(), Lang::En.source())
}

fn own(lang: Lang, text: &str, errors: &mut Vec<LocaleError>) -> Option<Vec<Entry>> {
    let entries = match parse(text) {
        Ok(entries) => entries,
        Err(reason) => {
            errors.push(LocaleError::NotJson {
                lang: lang.code(),
                reason,
            });
            return None;
        }
    };
    for (i, e) in entries.iter().enumerate() {
        if entries[..i].iter().any(|prev| prev.key == e.key) {
            errors.push(LocaleError::Duplicate {
                lang: lang.code(),
                key: e.key.clone(),
                line: e.line,
            });
        } else if i > 0 && entries[i - 1].key > e.key {
            errors.push(LocaleError::Unsorted {
                lang: lang.code(),
                key: e.key.clone(),
                line: e.line,
            });
        }
        if e.value.trim().is_empty() {
            errors.push(LocaleError::Empty {
                lang: lang.code(),
                key: e.key.clone(),
                line: e.line,
            });
        }
    }
    Some(entries)
}

fn against_reference(lang: Lang, ru: &[Entry], other: &[Entry], errors: &mut Vec<LocaleError>) {
    for r in ru {
        match other.iter().find(|o| o.key == r.key) {
            None => errors.push(LocaleError::Missing {
                lang: lang.code(),
                key: r.key.clone(),
            }),
            Some(o) => {
                let (expected, found) = (placeholders(&r.value), placeholders(&o.value));
                if expected != found {
                    errors.push(LocaleError::Placeholders {
                        lang: lang.code(),
                        key: r.key.clone(),
                        line: o.line,
                        expected,
                        found,
                    });
                }
            }
        }
    }
    for o in other {
        if !ru.iter().any(|r| r.key == o.key) {
            errors.push(LocaleError::Extra {
                lang: lang.code(),
                key: o.key.clone(),
                line: o.line,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RU: &str = "{\n  \"a.one\": \"Один {n}\",\n  \"b.two\": \"Два\"\n}";

    #[test]
    fn dictionaries_in_the_build_are_valid() {
        assert_eq!(check_embedded(), Vec::new());
    }

    #[test]
    fn matching_dictionaries_pass() {
        assert!(check(RU, "{\"a.one\": \"One {n}\", \"b.two\": \"Two\"}").is_empty());
    }

    #[test]
    fn every_rule_is_reported_with_language_key_and_line() {
        let en = "{\n  \"b.two\": \"Two\",\n  \"a.one\": \"One\",\n  \"a.one\": \" \",\n  \"c.extra\": \"x\"\n}";
        let messages: Vec<String> = check(RU, en).iter().map(ToString::to_string).collect();
        assert_eq!(
            messages,
            [
                "Неправильная локализация en: ключ a.one стоит не по алфавиту (строка 3)",
                "Неправильная локализация en: ключ a.one повторяется (строка 4)",
                "Неправильная локализация en: пустое значение у ключа a.one (строка 4)",
                "Неправильная локализация en: подстановки у ключа a.one не совпадают с ru.json: [] вместо [\"n\"] (строка 3)",
                "Неправильная локализация en: лишний ключ c.extra, его нет в ru.json (строка 5)",
            ]
        );
    }

    #[test]
    fn missing_key_and_broken_json_are_reported() {
        let missing = check(RU, "{\"a.one\": \"One {n}\"}");
        assert_eq!(
            missing,
            [LocaleError::Missing {
                lang: "en",
                key: "b.two".into()
            }]
        );
        assert!(matches!(
            check("{", "{}").as_slice(),
            [LocaleError::NotJson { lang: "ru", .. }]
        ));
    }
}
