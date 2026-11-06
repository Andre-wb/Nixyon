//! Имена спикеров по смыслу разговора: языковая модель на устройстве читает расшифровку
//! и подсказывает, как зовут тех, кого назвали по имени (представились или к ним
//! обратились). Команда ничего не сохраняет, а возвращает предложения: имена ставит
//! интерфейс обычным переименованием. Модель одна, поэтому работа занимает тот же слот
//! `SummaryRunning`, что и протокол встречи.

use nixyon_android::{SpeechExt, SummaryRequest};
use nixyon_core::Transcript;
use nixyon_pipeline::{apply_names, clean_name, name_is_grounded};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime, State};

use super::{NO_TRANSCRIPT, SummaryRunning, answer, llm_error, prompt};
use crate::transcription::{CommandError, last_transcript};

/// Подсказка модели. Контекст модели 4096 токенов, поэтому текст ниже урезается.
const SYSTEM: &str = "Ты определяешь имена участников разговора по расшифровке. \
Каждая реплика начинается с номера спикера в квадратных скобках. \
Ответь одним JSON-объектом без пояснений: ключ — номер спикера, значение — его имя \
в именительном падеже или null. \
Имя ставь только если оно прямо названо в тексте: человек сам представился или к нему \
обратились по имени. Если к кому-то обращаются по имени, это имя того, кто отвечает, \
а не того, кто говорит. Не выдумывай и не угадывай: если не уверен, пиши null. \
Пример ответа: {\"1\": \"Анна\", \"2\": null}";

/// Бюджет текста расшифровки в запросе, символы. С запасом под ответ и подсказку.
const MAX_CHARS: usize = 6000;
/// Дольше этого реплика в запрос не идёт: имена называют в начале реплики.
const MAX_TURN_CHARS: usize = 400;

/// Одно предложение имени для спикера.
#[derive(Debug, Serialize, PartialEq)]
pub struct SpeakerName {
    pub speaker: u32,
    pub name: String,
}

fn clip(text: &str) -> String {
    text.chars().take(MAX_TURN_CHARS).collect()
}

fn is_end(w: &str) -> bool {
    w.ends_with(['.', '?', '!', '…']) || w.chars().all(|c| matches!(c, '—' | '–' | '-'))
}

// Есть слово с заглавной буквы не в начале фразы: похоже на имя.
fn has_inner_capital(text: &str) -> bool {
    let mut sentence_start = true;
    for w in text.split_whitespace() {
        let capital = w.chars().find(|c| c.is_alphabetic()).is_some_and(char::is_uppercase);
        if !sentence_start && capital {
            return true;
        }
        sentence_start = is_end(w);
    }
    false
}

/// Запрос к модели. None, если называть нечего: у всех спикеров уже есть имена.
/// В запрос идёт начало разговора (там обычно знакомятся), а остаток бюджета отдаётся
/// репликам дальше по тексту, где есть слово, похожее на имя.
pub(super) fn user_prompt(t: &Transcript) -> Option<String> {
    let unnamed: Vec<String> = t
        .speakers
        .iter()
        .filter(|s| s.name.is_none())
        .map(|s| s.id.to_string())
        .collect();
    if unnamed.is_empty() {
        return None;
    }
    let lines: Vec<String> = t
        .turns
        .iter()
        .map(|turn| format!("[{}] {}\n", turn.speaker, clip(&turn.text)))
        .collect();
    let mut picked = vec![false; lines.len()];
    let mut used = 0;
    let head_budget = MAX_CHARS * 2 / 3;
    for (i, line) in lines.iter().enumerate() {
        let len = line.chars().count();
        if used + len > head_budget {
            break;
        }
        used += len;
        picked[i] = true;
    }
    for (i, turn) in t.turns.iter().enumerate() {
        let len = lines[i].chars().count();
        if picked[i] || !has_inner_capital(&turn.text) || used + len > MAX_CHARS {
            continue;
        }
        used += len;
        picked[i] = true;
    }
    let body: String = lines
        .iter()
        .zip(&picked)
        .filter(|(_, p)| **p)
        .map(|(line, _)| line.as_str())
        .collect();
    if body.is_empty() {
        return None;
    }
    Some(format!("Нужны имена спикеров: {}.\n\n{body}", unnamed.join(", ")))
}

/// Разбор ответа модели: JSON-объект «номер спикера: имя». Имя берётся, только если оно
/// выглядит как имя и действительно встречается в тексте разговора.
pub(super) fn parse(answer: &str, t: &Transcript) -> Vec<(u32, String)> {
    let (Some(from), Some(to)) = (answer.find('{'), answer.rfind('}')) else {
        return Vec::new();
    };
    if from >= to {
        return Vec::new();
    }
    let Ok(map) =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&answer[from..=to])
    else {
        return Vec::new();
    };
    map.into_iter()
        .filter_map(|(key, value)| {
            let id: u32 = key.trim().trim_start_matches("Спикер").trim().parse().ok()?;
            let name = clean_name(value.as_str()?)?;
            name_is_grounded(&name, &t.turns).then_some((id, name))
        })
        .collect()
}

/// Имена спикеров без имени по смыслу разговора. Пустой список, если называть некого,
/// модель ничего не нашла или ответ не прошёл проверку. Ничего не сохраняет.
#[tauri::command]
pub async fn suggest_speaker_names<R: Runtime>(
    app: AppHandle<R>,
    running: State<'_, SummaryRunning>,
) -> Result<Vec<SpeakerName>, CommandError> {
    let slot = running
        .0
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;
    let store = crate::vault::storage(&app).await?.store;
    let (transcript, user) = tauri::async_runtime::spawn_blocking(move || {
        let transcript = last_transcript::load(&store)
            .ok_or_else(|| CommandError::new(NO_TRANSCRIPT))?;
        let user = user_prompt(&transcript);
        Ok::<_, CommandError>((transcript, user))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))??;
    let Some(user) = user else {
        return Ok(Vec::new());
    };
    if slot.cancel.is_cancelled() {
        return Err(CommandError::new(CommandError::CANCELLED));
    }

    let reply = app
        .speech()
        .summarize(SummaryRequest {
            system: SYSTEM.to_owned(),
            user,
            max_tokens: prompt::MAX_TOKENS,
            on_progress: Channel::new(|_| Ok(())),
        })
        .await
        .map_err(llm_error)?;
    if slot.cancel.is_cancelled() {
        return Err(CommandError::new(CommandError::CANCELLED));
    }
    drop(slot);

    let Some(text) = answer::clean_answer(&reply.text) else {
        return Ok(Vec::new());
    };
    // Те же правила, что у имён из речи: только спикеры без имени и только свободные имена.
    let mut speakers = transcript.speakers.clone();
    apply_names(&mut speakers, parse(&text, &transcript));
    Ok(speakers
        .iter()
        .zip(&transcript.speakers)
        .filter(|(new, old)| new.name != old.name)
        .filter_map(|(new, _)| {
            Some(SpeakerName {
                speaker: new.id,
                name: new.name.clone()?,
            })
        })
        .collect())
}
