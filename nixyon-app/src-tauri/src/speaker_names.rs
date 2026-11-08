//! Подсказка имён участников из самого разговора: кто представился или к кому обратились
//! по имени. Модель и проверку её ответа держит Kotlin-плагин (SpeakerNames.kt); ядро
//! отдаёт ему участников и реплики расшифровки встречи. Имена только предлагаются,
//! подписывает пользователь.

use nixyon_android::{
    NameSuggestion, NamingSpeaker, NamingTurn, SpeakerNamesRequest, SpeechExt, SummaryProgress,
};
use nixyon_core::Transcript;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime, State};

use crate::summary::{SummaryRunning, llm_error};
use crate::transcription::CommandError;

/// Подсказки имён для встречи. Ход работы модели идёт в канал. Пока модель
/// составляет протокол, подсказка не начинается, и наоборот.
#[tauri::command]
pub async fn suggest_speaker_names<R: Runtime>(
    app: AppHandle<R>,
    running: State<'_, SummaryRunning>,
    meeting: String,
    on_progress: Channel<SummaryProgress>,
) -> Result<Vec<NameSuggestion>, CommandError> {
    let slot = running
        .0
        .begin()
        .ok_or_else(|| CommandError::new(CommandError::BUSY))?;
    let store = crate::vault::storage(&app).await?.store;
    let store_for_load = store.clone();
    let (id, transcript) = tauri::async_runtime::spawn_blocking(move || {
        crate::meetings::load(&store_for_load, &meeting)
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::ENGINE_FAILED, e))??;
    if slot.cancel.is_cancelled() {
        return Err(CommandError::new(CommandError::CANCELLED));
    }

    let (speakers, turns) = naming_input(&transcript);
    let forward = on_progress.clone();
    let reply = app
        .speech()
        .suggest_speaker_names(SpeakerNamesRequest {
            language: transcript.language,
            speakers,
            turns,
            on_progress: Channel::new(move |body| {
                if let InvokeResponseBody::Json(text) = body
                    && let Ok(p) = serde_json::from_str::<SummaryProgress>(&text)
                {
                    let _ = forward.send(p);
                }
                Ok(())
            }),
        })
        .await
        .map_err(llm_error)?;
    // Отмена могла прийти раньше, чем Kotlin начал работу, и не дойти до модели.
    if slot.cancel.is_cancelled() {
        return Err(CommandError::new(CommandError::CANCELLED));
    }
    // Подсказки ссылаются на номера спикеров и реплик этой встречи: если её не стало, пока
    // модель работала, отдавать их некому.
    let still_there =
        tauri::async_runtime::spawn_blocking(move || crate::meetings::exists(&store, &id))
            .await
            .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
    if !still_there {
        return Err(CommandError::new(crate::meetings::NO_TRANSCRIPT));
    }
    drop(slot);
    Ok(reply.suggestions)
}

/// Участники и реплики расшифровки для плагина. Пустое имя значит «Спикер N».
fn naming_input(transcript: &Transcript) -> (Vec<NamingSpeaker>, Vec<NamingTurn>) {
    let speakers = transcript
        .speakers
        .iter()
        .map(|s| NamingSpeaker {
            id: s.id,
            name: s
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_owned),
        })
        .collect();
    let turns = transcript
        .turns
        .iter()
        .map(|t| NamingTurn {
            id: t.id,
            speaker: t.speaker,
            text: t.text.clone(),
        })
        .collect();
    (speakers, turns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nixyon_core::{AudioInfo, EngineInfo, Speaker, SpeechLanguage, Turn};

    fn speaker(id: u32, name: Option<&str>) -> Speaker {
        Speaker {
            id,
            name: name.map(str::to_owned),
            talk_time_sec: 1.0,
            turns: 1,
            words: 1,
            age: None,
            gender: None,
        }
    }

    fn turn(id: u32, speaker: u32, text: &str) -> Turn {
        Turn {
            id,
            speaker,
            start: 0.0,
            end: 1.0,
            text: text.into(),
            words: None,
            flags: Vec::new(),
            emotion: None,
        }
    }

    #[test]
    fn speakers_keep_given_names_and_blank_ones_count_as_unnamed() {
        let transcript = Transcript {
            schema_version: 1,
            created_at: String::new(),
            audio: AudioInfo {
                file: "a".into(),
                duration_sec: 2.0,
                sample_rate: 16_000,
            },
            engine: EngineInfo {
                vad: String::new(),
                asr: String::new(),
                embedding: String::new(),
            },
            language: SpeechLanguage::Ru,
            speakers: vec![
                speaker(1, Some(" Анна ")),
                speaker(2, None),
                speaker(3, Some("  ")),
            ],
            turns: vec![turn(1, 1, "Борис, возьмёшь?"), turn(2, 2, "Возьму.")],
        };
        let (speakers, turns) = naming_input(&transcript);
        let names: Vec<Option<&str>> = speakers.iter().map(|s| s.name.as_deref()).collect();
        assert_eq!(names, [Some("Анна"), None, None]);
        assert_eq!(
            turns,
            [
                NamingTurn {
                    id: 1,
                    speaker: 1,
                    text: "Борис, возьмёшь?".into()
                },
                NamingTurn {
                    id: 2,
                    speaker: 2,
                    text: "Возьму.".into()
                },
            ]
        );
    }
}
