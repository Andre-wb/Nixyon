//! Слова-паразиты: языковая модель на телефоне убирает их из выбранных фраз или из всего
//! диалога. Фразы без таких слов модели не отправляются, поэтому лимит тратится только на
//! нужное. Ответ модели принимается, только если он не добавляет слов и не режет фразу
//! больше чем наполовину. Правки не сохраняются: интерфейс показывает «было» и «стало»,
//! замена идёт правкой `apply_turn_texts`.

use std::collections::{HashMap, HashSet};

use nixyon_android::{SpeechExt, SummaryRequest};
use nixyon_core::Turn;
use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime, State};

use crate::reprocessing::unquote;
use crate::summary::{NO_TRANSCRIPT, SummaryRunning, answer, llm_error};
use crate::transcription::{CommandError, last_transcript};

const UNKNOWN_TURN: &str = "unknown_turn";

/// Слова, которые убираются, если ничего не значат. Многословные записаны через пробел.
const FILLERS: &[&str] = &[
    "э",
    "эм",
    "ээ",
    "мм",
    "ну",
    "вот",
    "типа",
    "короче",
    "значит",
    "как бы",
    "так сказать",
    "это самое",
    "в общем",
];

/// Фраза длиннее этого не идёт в модель: её ответ должен оставаться в лимите.
const MAX_TURN_CHARS: usize = 1500;
/// Фразы одного запроса к модели: пачка, а не по одной фразе.
const BATCH_CHARS: usize = 2000;
/// Предел длины ответа на одну пачку.
const MAX_TOKENS: u32 = 1024;

/// Инструкция модели. Русская при любом языке интерфейса, как и остальные инструкции.
const SYSTEM: &str = "Ты убираешь слова-паразиты из реплик разговора. Слова-паразиты — \
«э», «эм», «ну», «вот», «типа», «короче», «значит», «как бы», «так сказать», «это самое», \
«в общем» и похожие, которые ничего не значат. Убирай их только там, где они ничего не \
несут. Не добавляй новых слов, не перефразируй, не меняй смысл, сохраняй язык и знаки \
препинания. Каждая реплика дана строкой «номер: текст». Ответь теми же строками «номер: \
текст» с очищенным текстом, без пояснений. Если паразитов в реплике нет, повтори её как \
есть.";

/// Ход работы: сколько пачек готово из скольких.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FillerProgress {
    pub done: u32,
    pub total: u32,
}

/// Предложенная правка одной фразы. `before` — текст как был при проверке.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FillerChange {
    pub id: u32,
    pub before: String,
    pub after: String,
}

/// Итог проверки. Ничего не сохраняется.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FillerResult {
    pub changes: Vec<FillerChange>,
    /// Фраз, которые проверила модель.
    pub checked: u32,
    /// Фраз со словами-паразитами, но слишком длинных для модели.
    pub skipped: u32,
}

/// Убирает слова-паразиты из реплик `ids` или, если `ids` нет, из всего диалога.
#[tauri::command]
pub async fn remove_filler_words<R: Runtime>(
    app: AppHandle<R>,
    running: State<'_, SummaryRunning>,
    ids: Option<Vec<u32>>,
    on_progress: Channel<FillerProgress>,
) -> Result<FillerResult, CommandError> {
    let slot = running
        .0
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;
    let store = crate::vault::storage(&app).await?.store;
    let plan = tauri::async_runtime::spawn_blocking(move || {
        let transcript =
            last_transcript::load(&store).ok_or_else(|| CommandError::new(NO_TRANSCRIPT))?;
        let turns = select(&transcript.turns, ids.as_deref())?;
        Ok::<_, CommandError>(plan(&turns))
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))??;
    if slot.cancel.is_cancelled() {
        return Err(CommandError::new(CommandError::CANCELLED));
    }

    let total = u32::try_from(plan.batches.len()).unwrap_or(u32::MAX);
    let mut result = FillerResult {
        skipped: plan.skipped,
        ..FillerResult::default()
    };
    for (done, batch) in plan.batches.iter().enumerate() {
        let _ = on_progress.send(FillerProgress {
            done: u32::try_from(done).unwrap_or(u32::MAX),
            total,
        });
        let reply = app
            .speech()
            .summarize(SummaryRequest {
                system: SYSTEM.to_owned(),
                user: batch_prompt(batch),
                max_tokens: MAX_TOKENS,
                on_progress: Channel::new(|_: InvokeResponseBody| Ok(())),
            })
            .await
            .map_err(llm_error)?;
        if slot.cancel.is_cancelled() {
            return Err(CommandError::new(CommandError::CANCELLED));
        }
        result.changes.extend(changes_from(batch, &reply.text));
        result.checked += u32::try_from(batch.len()).unwrap_or(u32::MAX);
    }
    let _ = on_progress.send(FillerProgress { done: total, total });
    drop(slot);
    Ok(result)
}

/// Реплики для проверки: выбранные (или все), в порядке диалога. Неизвестный номер —
/// ошибка, иначе выбор молча станет другим.
fn select(turns: &[Turn], ids: Option<&[u32]>) -> Result<Vec<Turn>, CommandError> {
    let Some(ids) = ids else {
        return Ok(turns.to_vec());
    };
    if let Some(missing) = ids.iter().find(|id| !turns.iter().any(|t| t.id == **id)) {
        log::warn!("реплика {missing} для слов-паразитов не найдена");
        return Err(CommandError::new(UNKNOWN_TURN));
    }
    Ok(turns
        .iter()
        .filter(|t| ids.contains(&t.id))
        .cloned()
        .collect())
}

/// Пачки фраз для модели. Фразы без слов-паразитов в пачки не попадают.
struct Plan {
    batches: Vec<Vec<(u32, String)>>,
    skipped: u32,
}

fn plan(turns: &[Turn]) -> Plan {
    let mut batches: Vec<Vec<(u32, String)>> = Vec::new();
    let mut size = 0;
    let mut skipped = 0;
    for turn in turns.iter().filter(|t| has_filler(&t.text)) {
        let len = turn.text.chars().count();
        if len > MAX_TURN_CHARS {
            skipped += 1;
            continue;
        }
        let fits = !batches.is_empty() && size + len <= BATCH_CHARS;
        if !fits {
            batches.push(Vec::new());
            size = 0;
        }
        size += len;
        if let Some(batch) = batches.last_mut() {
            batch.push((turn.id, turn.text.clone()));
        }
    }
    Plan { batches, skipped }
}

/// Слова в нижнем регистре через пробел: сравнение не зависит от знаков и регистра.
fn words(text: &str) -> String {
    text.to_lowercase()
        .replace('ё', "е")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn has_filler(text: &str) -> bool {
    let padded = format!(" {} ", words(text));
    FILLERS.iter().any(|f| padded.contains(&format!(" {f} ")))
}

/// Фразы пачки нумеруются по порядку 1, 2, 3…: так модели проще не сбиться, а номер
/// реплики в разговоре ей ни к чему.
fn batch_prompt(batch: &[(u32, String)]) -> String {
    batch
        .iter()
        .enumerate()
        .map(|(i, (_, text))| format!("{}: {text}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Разбор ответа: строка «номер: текст» на каждую фразу пачки. Фраза без строки или с
/// отвергнутым ответом правки не получает, остаётся как была.
fn changes_from(batch: &[(u32, String)], raw: &str) -> Vec<FillerChange> {
    let Some(body) = answer::clean_answer(raw) else {
        return Vec::new();
    };
    let lines: HashMap<usize, &str> = body
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_start_matches(['-', '*', '•']).trim();
            let (number, text) = line.split_once(':')?;
            Some((number.trim().parse::<usize>().ok()?, text))
        })
        .collect();
    batch
        .iter()
        .enumerate()
        .filter_map(|(i, (id, before))| {
            let after = clean_turn(before, lines.get(&(i + 1))?)?;
            (after != squash(before)).then(|| FillerChange {
                id: *id,
                before: before.clone(),
                after,
            })
        })
        .collect()
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Ответ модели на одну фразу. Отвергается, если в нём есть слово, которого не было во
/// фразе, если он длиннее фразы или если от фразы осталась меньше половины слов.
fn clean_turn(before: &str, reply: &str) -> Option<String> {
    let text: String = squash(reply).chars().filter(|c| !c.is_control()).collect();
    let text = unquote(text.trim());
    if text.is_empty() {
        return None;
    }
    let original = words(before);
    let known: HashSet<&str> = original.split(' ').collect();
    let normalized = words(text);
    let reply_words: Vec<&str> = normalized.split(' ').collect();
    if reply_words.iter().any(|w| !known.contains(w)) {
        return None;
    }
    let before_count = original.split(' ').count();
    let after_count = reply_words.len();
    if after_count > before_count || (before_count >= 4 && after_count * 2 < before_count) {
        return None;
    }
    Some(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(id: u32, text: &str) -> Turn {
        Turn {
            id,
            speaker: 1,
            start: f64::from(id),
            end: f64::from(id) + 1.0,
            text: text.into(),
            words: None,
            flags: Vec::new(),
        }
    }

    #[test]
    fn finds_fillers_as_whole_words_in_any_case() {
        assert!(has_filler("Ну, вот мы и пришли."));
        assert!(has_filler("Это, эм, не так."));
        assert!(has_filler("Мы, как бы, уже готовы"));
        assert!(!has_filler("Нужно было вернуться"));
        assert!(!has_filler("Вотум и номер"));
    }

    #[test]
    fn only_phrases_with_fillers_are_sent_and_long_ones_are_skipped() {
        let long = format!("ну {}", "слово ".repeat(MAX_TURN_CHARS / 6 + 1));
        let turns = [
            turn(1, "Привет, мир."),
            turn(2, "Ну, привет."),
            turn(3, &long),
            turn(4, "Значит, да."),
        ];
        let p = plan(&turns);
        assert_eq!(p.skipped, 1);
        let ids: Vec<u32> = p.batches.iter().flatten().map(|(id, _)| *id).collect();
        assert_eq!(ids, [2, 4]);
    }

    #[test]
    fn long_selection_is_split_into_batches() {
        let text = format!("ну {}", "а".repeat(900));
        let turns: Vec<Turn> = (1..=5).map(|id| turn(id, &text)).collect();
        let p = plan(&turns);
        assert_eq!(p.batches.len(), 3);
        assert_eq!(p.batches.iter().map(Vec::len).sum::<usize>(), 5);
    }

    #[test]
    fn selection_keeps_dialogue_order_and_rejects_unknown_ids() {
        let turns = [turn(1, "а"), turn(2, "б"), turn(3, "в")];
        let ids: Vec<u32> = select(&turns, Some(&[3, 1]))
            .unwrap()
            .iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(ids, [1, 3]);
        assert_eq!(select(&turns, None).unwrap().len(), 3);
        assert_eq!(select(&turns, Some(&[9])).unwrap_err().code, UNKNOWN_TURN);
    }

    #[test]
    fn reply_lines_become_changes_only_when_they_differ() {
        let batch = vec![
            (7, "Ну, это, эм, хорошо.".to_owned()),
            (9, "Ну, привет.".to_owned()),
            (12, "Вот, да.".to_owned()),
        ];
        let reply = "1: Это хорошо.\n2: Ну, привет.\nмусор";
        let changes = changes_from(&batch, reply);
        assert_eq!(
            changes,
            [FillerChange {
                id: 7,
                before: "Ну, это, эм, хорошо.".into(),
                after: "Это хорошо.".into(),
            }]
        );
    }

    #[test]
    fn model_reply_that_invents_words_is_rejected() {
        assert_eq!(clean_turn("Ну, привет.", "Здравствуй, привет."), None);
        assert_eq!(clean_turn("Ну, привет.", "Привет."), Some("Привет.".into()));
    }

    #[test]
    fn model_reply_that_cuts_most_of_the_phrase_is_rejected() {
        assert_eq!(clean_turn("ну вот мы и здесь", "мы"), None);
        assert_eq!(
            clean_turn("ну вот мы и здесь", "мы и здесь"),
            Some("мы и здесь".into())
        );
    }

    #[test]
    fn quotes_and_empty_answers_are_handled() {
        assert_eq!(clean_turn("ну да", "«да»"), Some("да".into()));
        assert_eq!(clean_turn("ну да", "  "), None);
    }
}
