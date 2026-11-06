//! Запрос к языковой модели: инструкция и текст встречи. Инструкция та же, что на замере
//! моделей (`scripts/llm-meeting-eval.sh`), текст встречи в том же виде, что TXT-экспорт.

use nixyon_core::Transcript;
use nixyon_locales::{Lang, LocaleError, export_labels};
use nixyon_render::render_txt;

/// Модель замерялась на русской инструкции и русских подписях встречи, поэтому они
/// русские при любом языке интерфейса.
pub const SYSTEM: &str = "Ты составляешь протокол встречи по её расшифровке. Пиши по-русски и только то, что есть в расшифровке, ничего не додумывай. Ответ строго в таком виде:
Краткое содержание: 3–5 предложений.
Решения: список, каждое с новой строки через «- ». Если решений нет, напиши «Решений нет».
Задачи: список, каждая с новой строки: имя исполнителя, двоеточие, что сделать, в скобках срок. Например: «- Пётр: отправить отчёт поставщику (до вторника)». Исполнителя пиши так, как он назван в расшифровке. Если срок не назван, пиши «(срок не назван)». Если задач нет, напиши «Задач нет».";

/// Предел длины ответа в токенах, как на замере.
pub const MAX_TOKENS: u32 = 1024;

/// Текст встречи для модели. Даты записи в нём нет: модели она не нужна.
pub fn meeting_text(transcript: &Transcript) -> Result<String, LocaleError> {
    Ok(render_txt(transcript, &export_labels(Lang::Ru)?, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_matches_the_one_the_models_were_measured_with() {
        let script = include_str!("../../../../scripts/llm-meeting-eval.sh");
        let start = script.find("SYSTEM='").unwrap() + "SYSTEM='".len();
        let end = start + script[start..].find('\'').unwrap();
        assert_eq!(SYSTEM, &script[start..end]);
        assert!(script.contains(&format!("max_tokens: {MAX_TOKENS}")));
    }
}
