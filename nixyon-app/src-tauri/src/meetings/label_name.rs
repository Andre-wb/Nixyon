//! Название метки из интерфейса. Проверяется только на входе: из индекса читается как
//! есть, чтобы смена правил не сделала прежний индекс нечитаемым.

use super::label::Label;

pub const MAX_LABEL_CHARS: usize = 40;

#[derive(Debug, PartialEq, Eq)]
pub enum LabelNameError {
    Empty,
    TooLong,
    ControlChars,
}

impl LabelNameError {
    pub fn code(&self) -> &'static str {
        match self {
            LabelNameError::Empty => "label_empty",
            LabelNameError::TooLong => "label_too_long",
            LabelNameError::ControlChars => "label_control_chars",
        }
    }
}

/// Пробелы по краям обрезаются; пустое название или из одних пробелов не принимается.
pub fn parse_label_name(text: &str) -> Result<String, LabelNameError> {
    let name = text.trim();
    if name.is_empty() {
        return Err(LabelNameError::Empty);
    }
    if name.chars().count() > MAX_LABEL_CHARS {
        return Err(LabelNameError::TooLong);
    }
    if name.chars().any(char::is_control) {
        return Err(LabelNameError::ControlChars);
    }
    Ok(name.to_owned())
}

/// Название уже есть у другой метки. Регистр не важен: «Отдел» и «отдел» одна метка.
/// `except`: метка, которую переименовывают, со своим прежним названием не сравнивается.
pub fn is_taken(labels: &[Label], name: &str, except: Option<u32>) -> bool {
    let name = name.to_lowercase();
    labels
        .iter()
        .any(|l| Some(l.id) != except && l.name.to_lowercase() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_trimmed_and_empty_or_blank_is_rejected() {
        assert_eq!(
            parse_label_name("  Отдел продаж "),
            Ok("Отдел продаж".into())
        );
        assert_eq!(parse_label_name(""), Err(LabelNameError::Empty));
        assert_eq!(
            parse_label_name("\u{a0} \u{3000}"),
            Err(LabelNameError::Empty)
        );
    }

    #[test]
    fn name_up_to_the_limit_is_accepted_and_longer_is_rejected() {
        let max = "Ж".repeat(MAX_LABEL_CHARS);
        assert_eq!(parse_label_name(&max), Ok(max.clone()));
        assert_eq!(
            parse_label_name(&format!("{max}Ж")),
            Err(LabelNameError::TooLong)
        );
    }

    #[test]
    fn control_chars_inside_the_name_are_rejected() {
        for bad in ["a\nb", "a\u{0}b", "a\tb"] {
            assert_eq!(
                parse_label_name(bad),
                Err(LabelNameError::ControlChars),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn taken_name_ignores_case_and_the_label_being_renamed() {
        let labels = [
            Label {
                id: 1,
                name: "Ёлка".into(),
            },
            Label {
                id: 2,
                name: "Отдел".into(),
            },
        ];
        assert!(is_taken(&labels, "отдел", None));
        assert!(is_taken(&labels, "ёЛКА", None));
        assert!(!is_taken(&labels, "отдел", Some(2)));
        assert!(!is_taken(&labels, "Склад", None));
    }
}
