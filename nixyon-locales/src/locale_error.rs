/// Сообщение для разработчика: какой язык, что не так и где.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LocaleError {
    #[error("Неправильная локализация {lang}: файл не читается как JSON-объект строк: {reason}")]
    NotJson { lang: &'static str, reason: String },
    #[error("Неправильная локализация {lang}: ключ {key} повторяется (строка {line})")]
    Duplicate {
        lang: &'static str,
        key: String,
        line: usize,
    },
    #[error("Неправильная локализация {lang}: ключ {key} стоит не по алфавиту (строка {line})")]
    Unsorted {
        lang: &'static str,
        key: String,
        line: usize,
    },
    #[error("Неправильная локализация {lang}: пустое значение у ключа {key} (строка {line})")]
    Empty {
        lang: &'static str,
        key: String,
        line: usize,
    },
    #[error("Неправильная локализация {lang}: не хватает ключа {key} из ru.json")]
    Missing { lang: &'static str, key: String },
    #[error(
        "Неправильная локализация {lang}: лишний ключ {key}, его нет в ru.json (строка {line})"
    )]
    Extra {
        lang: &'static str,
        key: String,
        line: usize,
    },
    #[error(
        "Неправильная локализация {lang}: подстановки у ключа {key} не совпадают с ru.json: {found:?} вместо {expected:?} (строка {line})"
    )]
    Placeholders {
        lang: &'static str,
        key: String,
        line: usize,
        expected: Vec<String>,
        found: Vec<String>,
    },
    #[error("Неправильная локализация {lang}: нет ключа {key}")]
    NoKey { lang: &'static str, key: String },
}
