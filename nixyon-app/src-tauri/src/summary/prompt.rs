//! Запрос к языковой модели: инструкция и текст встречи. Инструкция та же, что на замере
//! моделей (`scripts/llm-meeting-eval.sh`), текст встречи в том же виде, что TXT-экспорт.
//! Язык инструкции и подписей встречи берётся по языку речи встречи, а не интерфейса:
//! модель замерялась на русской инструкции и русских подписях, и русская встреча так и идёт
//! при любом языке интерфейса.

use nixyon_core::{SpeechLanguage, Transcript};
use nixyon_locales::{Lang, LocaleError, export_labels};
use nixyon_render::render_txt;

/// Инструкция для русской встречи.
pub const SYSTEM: &str = "Ты составляешь протокол встречи по её расшифровке. Пиши по-русски и только то, что есть в расшифровке, ничего не додумывай. Ответ строго в таком виде:
Краткое содержание: 3–5 предложений.
Решения: список, каждое с новой строки через «- ». Если решений нет, напиши «Решений нет».
Задачи: список, каждая с новой строки: имя исполнителя, двоеточие, что сделать, в скобках срок. Например: «- Пётр: отправить отчёт поставщику (до вторника)». Исполнителя пиши так, как он назван в расшифровке. Если срок не назван, пиши «(срок не назван)». Если задач нет, напиши «Задач нет».";

/// Инструкция для английской встречи: перевод русской один в один. Качество ответа на
/// ней не замерено: английских встреч с эталоном в замере нет.
pub const SYSTEM_EN: &str = "You write the minutes of a meeting from its transcript. Write in English and only what is in the transcript, do not make anything up. Answer strictly in this format:
Summary: 3–5 sentences.
Decisions: a list, each on a new line starting with \"- \". If there are no decisions, write \"No decisions\".
Tasks: a list, each on a new line: the assignee's name, a colon, what to do, the deadline in parentheses. For example: \"- Peter: send the report to the supplier (by Tuesday)\". Write the assignee as they are named in the transcript. If no deadline is named, write \"(no deadline named)\". If there are no tasks, write \"No tasks\".";

/// Предел длины ответа в токенах, как на замере.
pub const MAX_TOKENS: u32 = 1024;

/// Инструкция на языке речи встречи.
pub fn system(language: SpeechLanguage) -> &'static str {
    match language {
        SpeechLanguage::Ru => SYSTEM,
        SpeechLanguage::En => SYSTEM_EN,
    }
}

/// Текст встречи для модели с подписями на языке речи встречи. Даты записи в нём нет:
/// модели она не нужна.
pub fn meeting_text(transcript: &Transcript) -> Result<String, LocaleError> {
    let lang = match transcript.language {
        SpeechLanguage::Ru => Lang::Ru,
        SpeechLanguage::En => Lang::En,
    };
    Ok(render_txt(transcript, &export_labels(lang)?, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Значение переменной оболочки в одинарных кавычках; '"'"' внутри даёт одну кавычку.
    fn shell_value(script: &str, name: &str) -> String {
        let marker = format!("\n{name}='");
        let start = script.find(&marker).unwrap() + marker.len();
        let mut value = String::new();
        let mut rest = &script[start..];
        loop {
            let end = rest.find('\'').unwrap();
            value.push_str(&rest[..end]);
            rest = &rest[end..];
            match rest.strip_prefix("'\"'\"'") {
                Some(after) => {
                    value.push('\'');
                    rest = after;
                }
                None => return value,
            }
        }
    }

    #[test]
    fn instruction_matches_the_one_the_models_were_measured_with() {
        let script = include_str!("../../../../scripts/llm-meeting-eval.sh");
        assert_eq!(SYSTEM, shell_value(script, "SYSTEM"));
        assert_eq!(SYSTEM_EN, shell_value(script, "SYSTEM_EN"));
        assert!(script.contains(&format!("max_tokens: {MAX_TOKENS}")));
    }

    #[test]
    fn instruction_follows_the_language_of_the_meeting() {
        assert_eq!(system(SpeechLanguage::Ru), SYSTEM);
        assert_eq!(system(SpeechLanguage::En), SYSTEM_EN);
        assert!(SYSTEM_EN.contains("Write in English"));
    }
}
