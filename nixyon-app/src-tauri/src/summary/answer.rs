//! Ответ модели: недоверенный текст. Интерфейс показывает его только как текст.

/// Ответ без служебного блока рассуждений, если модель его всё же написала, и без
/// пустых краёв. None: содержательного текста нет.
pub fn clean_answer(raw: &str) -> Option<String> {
    let body = raw.rsplit_once("</think>").map_or(raw, |(_, after)| after);
    let text = body.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_is_trimmed_and_thinking_block_is_dropped() {
        assert_eq!(
            clean_answer("\n\nКраткое содержание: всё.\n").as_deref(),
            Some("Краткое содержание: всё.")
        );
        assert_eq!(
            clean_answer("<think>\nдумаю\n</think>\n\nЗадач нет.").as_deref(),
            Some("Задач нет.")
        );
        assert_eq!(clean_answer("  \n"), None);
        assert_eq!(clean_answer("<think>x</think>"), None);
    }
}
