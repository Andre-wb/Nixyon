#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Ru, Lang::En];

    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
        }
    }

    /// Неизвестный язык даёт None: выбор запасного (русского) делает вызывающий.
    pub fn from_code(code: &str) -> Option<Lang> {
        let primary = code.split(['-', '_']).next()?.to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == primary)
    }

    pub(crate) fn source(self) -> &'static str {
        match self {
            Lang::Ru => include_str!("../../nixyon-app/ui/locales/ru.json"),
            Lang::En => include_str!("../../nixyon-app/ui/locales/en.json"),
        }
    }
}
