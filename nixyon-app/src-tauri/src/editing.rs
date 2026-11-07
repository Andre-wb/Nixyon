//! Правки последней расшифровки: переименование и слияние спикеров, исправление текста
//! и эмоции реплики, удаление реплики. После правки
//! расшифровка сохраняется и возвращается интерфейсу целиком.

use nixyon_core::{Transcript, TurnEmotion};
use nixyon_pipeline::{
    EditError, delete_turn as remove_turn, edit_turn_text as edit_text, merge_centers,
    merge_speakers as merge, rename_speaker as rename, set_turn_emotion as set_emotion,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::transcription::{CommandError, last_centers, last_transcript};
use crate::vault::FilesStore;

const NO_TRANSCRIPT: &str = "no_transcript";

fn edit_code(e: &EditError) -> &'static str {
    match e {
        EditError::UnknownSpeaker(_) => "unknown_speaker",
        EditError::SameSpeaker => "same_speaker",
        EditError::NameTooLong { .. } => "name_too_long",
        EditError::NameHasControlChars => "name_control_chars",
        EditError::UnknownTurn(_) => "unknown_turn",
        EditError::TextEmpty => "text_empty",
        EditError::TextTooLong { .. } => "text_too_long",
        EditError::TextHasControlChars => "text_control_chars",
        EditError::LastTurn => "last_turn",
    }
}

/// Загружает последнюю расшифровку, применяет правку и сохраняет. Ошибка правки
/// оставляет файл как был.
pub fn apply(
    store: &FilesStore,
    edit: impl FnOnce(&mut Transcript) -> Result<(), EditError>,
) -> Result<Transcript, CommandError> {
    let mut transcript =
        last_transcript::load(store).ok_or_else(|| CommandError::new(NO_TRANSCRIPT))?;
    edit(&mut transcript).map_err(|e| CommandError::new(edit_code(&e)))?;
    last_transcript::save(store, &transcript)
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
    Ok(transcript)
}

/// Слияние спикеров вместе с их центрами голосов: центры смешиваются с весом по времени
/// речи до слияния. Если новые центры не записались, старые стираются, чтобы по ним не
/// запомнить чужой голос.
pub fn merge_with_centers(
    store: &FilesStore,
    keep: u32,
    absorb: u32,
) -> Result<Transcript, CommandError> {
    let mut weights = (0.0, 0.0);
    let transcript = apply(store, |t| {
        let talk = |id| t.speaker(id).map_or(0.0, |s| s.talk_time_sec);
        weights = (talk(keep), talk(absorb));
        merge(t, keep, absorb)
    })?;
    if let Some(mut centers) = last_centers::load(store) {
        merge_centers(&mut centers, keep, absorb, weights.0, weights.1);
        if let Err(e) = last_centers::save(store, &centers) {
            log::warn!("центры голосов после слияния не записались: {e}");
            if let Err(e) = last_centers::clear(store) {
                log::error!("устаревшие центры голосов не стёрлись: {e}");
            }
        }
    }
    Ok(transcript)
}

async fn files_store<R: Runtime>(app: &AppHandle<R>) -> Result<FilesStore, CommandError> {
    Ok(crate::vault::storage(app).await?.store)
}

async fn run<R: Runtime>(
    app: AppHandle<R>,
    edit: impl FnOnce(&mut Transcript) -> Result<(), EditError> + Send + 'static,
) -> Result<Transcript, CommandError> {
    let store = files_store(&app).await?;
    tauri::async_runtime::spawn_blocking(move || apply(&store, edit))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

/// Пустое имя возвращает спикеру подпись «Спикер N».
#[tauri::command]
pub async fn rename_speaker<R: Runtime>(
    app: AppHandle<R>,
    id: u32,
    name: String,
) -> Result<Transcript, CommandError> {
    run(app, move |t| rename(t, id, &name)).await
}

/// Реплики `absorb` переходят к `keep`, `absorb` пропадает. Отменить нельзя: интерфейс
/// спрашивает подтверждение.
#[tauri::command]
pub async fn merge_speakers<R: Runtime>(
    app: AppHandle<R>,
    keep: u32,
    absorb: u32,
) -> Result<Transcript, CommandError> {
    let store = files_store(&app).await?;
    tauri::async_runtime::spawn_blocking(move || merge_with_centers(&store, keep, absorb))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

/// Исправленный текст реплики; время слов у неё пропадает, тап играет её с начала.
/// Текст в лог не пишется: это слова из разговора.
#[tauri::command]
pub async fn edit_turn_text<R: Runtime>(
    app: AppHandle<R>,
    id: u32,
    text: String,
) -> Result<Transcript, CommandError> {
    run(app, move |t| edit_text(t, id, &text)).await
}

/// Реплика удаляется целиком вместе со временем. Отменить нельзя: интерфейс спрашивает
/// подтверждение.
#[tauri::command]
pub async fn delete_turn<R: Runtime>(
    app: AppHandle<R>,
    id: u32,
) -> Result<Transcript, CommandError> {
    run(app, move |t| remove_turn(t, id)).await
}

/// Правка текста реплики из листа слов-паразитов. Замена идёт, только если текст реплики
/// всё ещё такой, каким был при проверке: реплику могли поправить вручную.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnTextChange {
    pub id: u32,
    pub before: String,
    pub after: String,
}

/// Расшифровка после замены и сколько правок не встало: их реплики поправили вручную
/// или удалили, пока шла проверка.
#[derive(Serialize)]
pub struct AppliedTexts {
    pub transcript: Transcript,
    pub skipped: u32,
}

#[tauri::command]
pub async fn apply_turn_texts<R: Runtime>(
    app: AppHandle<R>,
    changes: Vec<TurnTextChange>,
) -> Result<AppliedTexts, CommandError> {
    let store = files_store(&app).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut skipped = 0;
        let transcript = apply(&store, |t| {
            skipped = apply_changes(t, &changes)?;
            Ok(())
        })?;
        Ok(AppliedTexts {
            transcript,
            skipped,
        })
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

/// Заменяет текст реплик, текст которых не менялся с проверки. Возвращает число пропущенных.
fn apply_changes(t: &mut Transcript, changes: &[TurnTextChange]) -> Result<u32, EditError> {
    let mut skipped = 0;
    for change in changes {
        let current = t
            .turns
            .iter()
            .any(|x| x.id == change.id && x.text == change.before);
        if current {
            edit_text(t, change.id, &change.after)?;
        } else {
            skipped += 1;
        }
    }
    Ok(skipped)
}

/// Эмоция реплики из списка выбора; None убирает её. Неизвестный ключ эмоции отвергает
/// сам разбор аргументов.
#[tauri::command]
pub async fn set_turn_emotion<R: Runtime>(
    app: AppHandle<R>,
    id: u32,
    emotion: Option<TurnEmotion>,
) -> Result<Transcript, CommandError> {
    run(app, move |t| set_emotion(t, id, emotion)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(id: u32, before: &str, after: &str) -> TurnTextChange {
        TurnTextChange {
            id,
            before: before.into(),
            after: after.into(),
        }
    }

    #[test]
    fn turn_texts_changed_by_hand_are_skipped_and_counted() {
        let mut t = sample();
        let first = t.turns[0].clone();
        let second = t.turns[1].clone();
        let changes = [
            change(first.id, &first.text, "новый текст"),
            change(second.id, "уже другой текст", "не встанет"),
        ];
        assert_eq!(apply_changes(&mut t, &changes), Ok(1));
        assert_eq!(t.turns[0].text, "новый текст");
        assert_eq!(t.turns[1].text, second.text);
    }

    #[test]
    fn merging_speakers_merges_their_voice_centers_by_talk_time() {
        let d = dir("centers");
        let mut t = sample();
        t.speakers[0].talk_time_sec = 3.0;
        t.speakers[1].talk_time_sec = 1.0;
        last_transcript::save(&d, &t).unwrap();
        last_centers::save(&d, &[(1, vec![1.0, 0.0]), (2, vec![0.0, 1.0])]).unwrap();
        let merged = merge_with_centers(&d, 1, 2).unwrap();
        assert_eq!(merged.speakers.len(), 1);
        let centers = last_centers::load(&d).unwrap();
        assert_eq!(centers.len(), 1);
        assert_eq!(centers[0].0, 1);
        assert!(centers[0].1[0] > centers[0].1[1]);
        assert_eq!(
            merge_with_centers(&d, 1, 1).unwrap_err().code,
            "same_speaker"
        );
        assert_eq!(last_centers::load(&d).unwrap().len(), 1);
    }

    #[test]
    fn merging_without_centers_still_merges_speakers() {
        let d = dir("nocenters");
        last_transcript::save(&d, &sample()).unwrap();
        assert_eq!(merge_with_centers(&d, 1, 2).unwrap().speakers.len(), 1);
        assert_eq!(last_centers::load(&d), None);
    }
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION, Speaker, Turn};

    fn sample() -> Transcript {
        let speaker = |id| Speaker {
            id,
            name: None,
            talk_time_sec: 2.0,
            turns: 1,
            words: 1,
            age: None,
            gender: None,
        };
        let turn = |id, speaker, start: f64| Turn {
            id,
            speaker,
            start,
            end: start + 2.0,
            text: "да".into(),
            words: None,
            flags: Vec::new(),
            emotion: None,
        };
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: "2026-10-29T10:00:00Z".into(),
            audio: AudioInfo {
                file: "a.wav".into(),
                duration_sec: 10.0,
                sample_rate: 16_000,
            },
            engine: EngineInfo {
                vad: "v".into(),
                asr: "a".into(),
                embedding: "e".into(),
            },
            speakers: vec![speaker(1), speaker(2)],
            turns: vec![turn(1, 1, 0.0), turn(2, 2, 3.0)],
        }
    }

    fn dir(name: &str) -> crate::vault::FilesStore {
        crate::vault::test_store(&format!("edit-{name}"))
    }

    #[test]
    fn edits_are_saved_and_bad_ones_leave_the_file_alone() {
        let d = dir("save");
        assert_eq!(apply(&d, |_| Ok(())).unwrap_err().code, NO_TRANSCRIPT);
        last_transcript::save(&d, &sample()).unwrap();
        let renamed = apply(&d, |t| rename(t, 2, "Анна")).unwrap();
        assert_eq!(renamed.speakers[1].name.as_deref(), Some("Анна"));
        assert_eq!(
            apply(&d, |t| rename(t, 9, "x")).unwrap_err().code,
            "unknown_speaker"
        );
        assert_eq!(
            apply(&d, |t| merge(t, 1, 1)).unwrap_err().code,
            "same_speaker"
        );
        assert_eq!(
            apply(&d, |t| rename(t, 1, &"я".repeat(65)))
                .unwrap_err()
                .code,
            "name_too_long"
        );
        assert_eq!(
            apply(&d, |t| edit_text(t, 1, " ")).unwrap_err().code,
            "text_empty"
        );
        assert_eq!(
            apply(&d, |t| edit_text(t, 9, "нет")).unwrap_err().code,
            "unknown_turn"
        );
        let edited = apply(&d, |t| edit_text(t, 1, "Да, конечно.")).unwrap();
        assert_eq!(edited.turns[0].text, "Да, конечно.");
        let merged = apply(&d, |t| merge(t, 2, 1)).unwrap();
        assert_eq!(merged.speakers.len(), 1);
        let reloaded = last_transcript::load(&d).unwrap();
        assert_eq!(reloaded.speakers[0].name.as_deref(), Some("Анна"));
        assert!(reloaded.turns.iter().all(|t| t.speaker == 2));
        assert_eq!(reloaded.turns[0].text, "Да, конечно.");
    }
}
