//! Своё название встречи из интерфейса. Проверяется только на входе: из индекса название
//! читается как есть, чтобы смена правил не сделала прежний индекс нечитаемым.

pub const MAX_TITLE_CHARS: usize = 100;

#[derive(Debug, PartialEq, Eq)]
pub enum TitleError {
    TooLong,
    ControlChars,
}

impl TitleError {
    pub fn code(&self) -> &'static str {
        match self {
            TitleError::TooLong => "title_too_long",
            TitleError::ControlChars => "title_control_chars",
        }
    }
}

/// Пробелы по краям обрезаются. Пустое название или из одних пробелов даёт `None`: встреча
/// снова называется датой и временем.
pub fn parse_title(text: &str) -> Result<Option<String>, TitleError> {
    let title = text.trim();
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(TitleError::TooLong);
    }
    if title.chars().any(char::is_control) {
        return Err(TitleError::ControlChars);
    }
    Ok((!title.is_empty()).then(|| title.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_blank_title_brings_back_the_default() {
        assert_eq!(parse_title(""), Ok(None));
        assert_eq!(parse_title("   "), Ok(None));
        assert_eq!(parse_title("\u{a0}\u{3000} "), Ok(None));
    }

    #[test]
    fn title_is_trimmed_and_inner_spaces_stay() {
        assert_eq!(
            parse_title("  Планёрка  отдела  "),
            Ok(Some("Планёрка  отдела".into()))
        );
    }

    #[test]
    fn title_up_to_the_limit_is_accepted_and_longer_is_rejected() {
        let max = "Ж".repeat(MAX_TITLE_CHARS);
        assert_eq!(parse_title(&max), Ok(Some(max.clone())));
        assert_eq!(parse_title(&format!("  {max}  ")), Ok(Some(max.clone())));
        assert_eq!(parse_title(&format!("{max}Ж")), Err(TitleError::TooLong));
        assert_eq!(parse_title(&"я".repeat(100_000)), Err(TitleError::TooLong));
    }

    #[test]
    fn control_chars_inside_the_title_are_rejected() {
        for bad in ["a\nb", "a\u{0}b", "a\tb", "a\u{1b}b"] {
            assert_eq!(parse_title(bad), Err(TitleError::ControlChars), "{bad:?}");
        }
        assert_eq!(parse_title("\nПланёрка\n"), Ok(Some("Планёрка".into())));
    }
}
