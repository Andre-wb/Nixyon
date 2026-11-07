//! Голоса участников: образец голоса записывается, по нему считается эмбеддинг, звук
//! удаляется. По голосам расшифровка сама ставит имена спикеров. Имена и эмбеддинги
//! в лог не пишутся: это личные данные. Голос можно запомнить и по встрече: центр голоса
//! спикера встречи сохраняется под его именем.

mod add_sample;
mod forget_meeting;
mod meeting_center;
mod meetings_of_voice;
mod remember;
mod store;
mod voice_meeting;
mod voice_name;
mod voice_record;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use nixyon_android::{SampleRequest, SpeechExt, StoragePaths};
use nixyon_audio::convert_reader_to_wav16k;
use nixyon_core::{MAX_SAMPLE_SEC, MIN_SAMPLE_SPEECH_SEC, VoiceSample, rfc3339_utc};
use serde::Serialize;
use tauri::{AppHandle, Runtime};

use crate::meetings::{centers_file, edit_lock};
use crate::transcription::CommandError;
use crate::transcription::allowed_path::resolve_input;
use crate::transcription::temp_files::TempFiles;
use crate::vault::{FilesStore, Storage};
use add_sample::add_sample;
use forget_meeting::forget_meeting;
use meeting_center::MeetingCenter;
use meetings_of_voice::meetings_of_voice;
use remember::remember;
use voice_meeting::VoiceMeeting;
use voice_record::VoiceRecord;

const UNKNOWN_SPEAKER: &str = "unknown_speaker";
const SPEAKER_UNNAMED: &str = "speaker_unnamed";
const NO_VOICE_CENTER: &str = "no_voice_center";

/// Голос в списке для интерфейса: без эмбеддинга.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceItem {
    pub name: String,
    pub created_at: String,
    /// Есть ли записанный образец; без него голос знают только по встречам.
    pub has_sample: bool,
    /// Встречи архива, на которых голос узнан, новые сверху; их число и есть «По встречам».
    pub meetings: Vec<VoiceMeeting>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoicesView {
    pub voices: Vec<VoiceItem>,
    /// Наибольшая длина записи образца: интерфейс останавливает запись сам.
    pub max_sample_sec: f64,
    /// Индекс встреч не прочитан: списки встреч у голосов пустые, причина в логе.
    pub meetings_failed: bool,
}

/// Голоса со встречами архива: индекс читается один раз на весь список.
fn view(files: &FilesStore, voices: &[VoiceRecord]) -> VoicesView {
    let (archive, meetings_failed) = match crate::meetings::archived(files) {
        Ok(archive) => (archive, false),
        Err(e) => {
            log::warn!("индекс встреч не читается, у голосов нет списка встреч: {e}");
            (Vec::new(), true)
        }
    };
    VoicesView {
        voices: voices
            .iter()
            .map(|v| VoiceItem {
                name: v.name.clone(),
                created_at: v.created_at.clone(),
                has_sample: v.embedding.is_some(),
                meetings: meetings_of_voice(&v.centers, &archive),
            })
            .collect(),
        max_sample_sec: MAX_SAMPLE_SEC,
        meetings_failed,
    }
}

/// Голоса для расшифровки: образец и центры встреч каждого человека под его именем,
/// конвейер считает их одним человеком. Берутся только посчитанные моделью голоса
/// `voice_model`: эмбеддинги другой модели сравнивать с записью нельзя. Битый файл голосов
/// не мешает расшифровке: имён просто нет.
pub fn samples(store: &FilesStore, voice_model: &str) -> Vec<VoiceSample> {
    match store::load(store) {
        Ok(voices) => voices
            .into_iter()
            .flat_map(|v| voice_samples(v, voice_model))
            .collect(),
        Err(e) => {
            log::warn!("голоса не читаются, расшифровка без имён: {e}");
            Vec::new()
        }
    }
}

fn voice_samples(voice: VoiceRecord, voice_model: &str) -> Vec<VoiceSample> {
    let same = |model: &Option<String>| model.as_deref() == Some(voice_model);
    let name = voice.name;
    let sample = voice.embedding.filter(|_| same(&voice.voice_model));
    sample
        .into_iter()
        .chain(
            voice
                .centers
                .into_iter()
                .filter(|c| same(&c.voice_model))
                .map(|c| c.embedding),
        )
        .map(|embedding| VoiceSample {
            name: name.clone(),
            embedding,
        })
        .collect()
}

/// Имена сохранённых голосов: они идут в горячие слова распознавания.
pub fn names(store: &FilesStore) -> Vec<String> {
    match store::load(store) {
        Ok(voices) => voices.into_iter().map(|v| v.name).collect(),
        Err(e) => {
            log::warn!("голоса не читаются, имена не идут в горячие слова: {e}");
            Vec::new()
        }
    }
}

async fn storage<R: Runtime>(app: &AppHandle<R>) -> Result<Storage, CommandError> {
    crate::vault::storage(app).await
}

fn load(files: &FilesStore) -> Result<Vec<VoiceRecord>, CommandError> {
    store::load(files).map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
}

/// Изменение голосов под блокировкой файла; ошибка изменения оставляет файл как был.
fn update<T>(
    files: &FilesStore,
    change: impl FnOnce(&mut Vec<VoiceRecord>) -> Result<T, CommandError>,
) -> Result<T, CommandError> {
    store::update(files, change)
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

// Запись образца берётся только из папок приложения, как запись для расшифровки.
fn recording(dirs: &StoragePaths, path: &str) -> Result<PathBuf, CommandError> {
    let roots: Vec<PathBuf> = [
        Some(&dirs.files),
        Some(&dirs.cache),
        dirs.external_files.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .collect();
    resolve_input(path, &roots).ok_or_else(|| {
        log::warn!("образец голоса: путь вне папок приложения или не файл");
        CommandError::new(CommandError::BAD_PATH)
    })
}

#[tauri::command]
pub async fn list_voices<R: Runtime>(app: AppHandle<R>) -> Result<VoicesView, CommandError> {
    let store = storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || load(&store).map(|v| view(&store, &v)))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

/// Голос по записанному образцу: новый или голос, запомненный только по встречам.
/// Запись образца удаляется при любом исходе.
#[tauri::command]
pub async fn add_voice<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    path: String,
) -> Result<VoicesView, CommandError> {
    let Storage {
        store: files,
        paths: dirs,
    } = storage(&app).await?;
    let input = recording(&dirs, &path)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let wav = Path::new(&dirs.cache).join(format!("nixyon-sample-{stamp}.wav"));
    let _temp = TempFiles::new(vec![input.clone(), wav.clone()]);

    let name = voice_name::clean_name(&name)
        .ok_or_else(|| CommandError::new(CommandError::BAD_VOICE_NAME))?;
    if voice_name::has_sample(&load(&files)?, &name) {
        return Err(CommandError::new(CommandError::VOICE_NAME_TAKEN));
    }

    let (from, to) = (input.clone(), wav.clone());
    let key = std::sync::Arc::clone(files.key());
    tauri::async_runtime::spawn_blocking(move || {
        convert_reader_to_wav16k(crate::vault::open_read(&from, &key)?, &to)
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::BAD_AUDIO, e))?
    .map_err(|e| CommandError::logged(CommandError::BAD_AUDIO, e))?;
    let sample = app
        .speech()
        .sample_embedding(SampleRequest {
            wav_path: wav.to_string_lossy().into_owned(),
            min_speech_sec: MIN_SAMPLE_SPEECH_SEC,
        })
        .await
        .map_err(|e| match e.rejected_code() {
            Some(code @ (CommandError::SAMPLE_TOO_SHORT | CommandError::BUSY)) => {
                CommandError::new(code)
            }
            Some(code) => CommandError::logged(code, &e),
            None => CommandError::logged(CommandError::ENGINE_FAILED, &e),
        })?;

    let created_at = rfc3339_utc(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    );
    tauri::async_runtime::spawn_blocking(move || {
        // Файл перечитывается: пока считался эмбеддинг, список мог измениться.
        update(&files, |voices| {
            add_sample(
                voices,
                &name,
                sample.embedding,
                sample.voice_model,
                created_at,
            )
            .map_err(|_| CommandError::new(CommandError::VOICE_NAME_TAKEN))?;
            Ok(view(&files, voices))
        })
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[tauri::command]
pub async fn delete_voice<R: Runtime>(
    app: AppHandle<R>,
    name: String,
) -> Result<VoicesView, CommandError> {
    let files = storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || {
        update(&files, |voices| {
            let before = voices.len();
            voices.retain(|v| v.name != name);
            if voices.len() == before {
                return Err(CommandError::new(CommandError::UNKNOWN_VOICE));
            }
            Ok(view(&files, voices))
        })
    })
    .await
    .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

/// Запоминает голос спикера встречи под его именем: центр голоса с этой встречи
/// добавляется к голосу с таким именем или становится новым голосом без образца.
/// Под блокировкой правок встречи: удаление встречи не может пройти между чтением её
/// центров и записью голоса и оставить в библиотеке центр уже удалённой встречи.
pub fn remember_speaker(
    files: &FilesStore,
    meeting: &str,
    speaker: u32,
    remembered_at: String,
) -> Result<VoicesView, CommandError> {
    edit_lock::locked(|| remember_unlocked(files, meeting, speaker, remembered_at))
}

fn remember_unlocked(
    files: &FilesStore,
    meeting: &str,
    speaker: u32,
    remembered_at: String,
) -> Result<VoicesView, CommandError> {
    let (id, transcript) = crate::meetings::load(files, meeting)?;
    let named = transcript
        .speaker(speaker)
        .ok_or_else(|| CommandError::new(UNKNOWN_SPEAKER))?;
    let name = named
        .name
        .as_deref()
        .and_then(voice_name::clean_name)
        .ok_or_else(|| CommandError::new(SPEAKER_UNNAMED))?;
    let embedding = centers_file::load(files, &id)
        .and_then(|centers| centers.into_iter().find(|(id, _)| *id == speaker))
        .map(|(_, embedding)| embedding)
        .ok_or_else(|| CommandError::new(NO_VOICE_CENTER))?;
    update(files, |voices| {
        remember(
            voices,
            &name,
            MeetingCenter {
                meeting: transcript.created_at.clone(),
                embedding,
                remembered_at,
                // Центр посчитан той же моделью голоса, что и вся расшифровка.
                voice_model: Some(transcript.engine.embedding.clone()),
            },
        );
        Ok(view(files, voices))
    })
}

/// Убирает из библиотеки голосов центры удалённой встречи. Отказ изменения невозможен,
/// ошибка только у чтения или записи файла.
pub fn forget_meeting_centers(files: &FilesStore, meeting: &str) -> std::io::Result<()> {
    store::update(files, |voices| {
        forget_meeting(voices, meeting);
        Ok::<_, std::convert::Infallible>(())
    })
    .map(|_| ())
}

#[tauri::command]
pub async fn remember_voice<R: Runtime>(
    app: AppHandle<R>,
    meeting: String,
    speaker: u32,
) -> Result<VoicesView, CommandError> {
    let files = storage(&app).await?.store;
    let now = rfc3339_utc(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    );
    tauri::async_runtime::spawn_blocking(move || remember_speaker(&files, &meeting, speaker, now))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

/// Удаляет записанный образец, если голос не сохраняют: звук голоса не должен
/// оставаться на устройстве.
#[tauri::command]
pub async fn discard_voice_recording<R: Runtime>(
    app: AppHandle<R>,
    path: String,
) -> Result<(), CommandError> {
    let dirs = storage(&app).await?.paths;
    let input = recording(&dirs, &path)?;
    std::fs::remove_file(&input).map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::NO_TRANSCRIPT;
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION, Speaker, Transcript};

    const M: &str = "2026-10-30T09:00:00Z";

    fn dir(name: &str) -> crate::vault::FilesStore {
        crate::vault::test_store(&format!("remember-{name}"))
    }

    fn transcript(names: &[Option<&str>]) -> Transcript {
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: M.into(),
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
            language: nixyon_core::SpeechLanguage::Ru,
            speakers: names
                .iter()
                .zip(1..)
                .map(|(name, id)| Speaker {
                    id,
                    name: name.map(String::from),
                    talk_time_sec: 1.0,
                    turns: 1,
                    words: 1,
                    age: None,
                    gender: None,
                })
                .collect(),
            turns: Vec::new(),
        }
    }

    #[test]
    fn named_speaker_center_is_remembered_and_used_for_next_transcripts() {
        let d = dir("ok");
        let id = crate::meetings::test_meeting(&d, &transcript(&[Some("Анна"), None]));
        centers_file::save(&d, &id, &[(1, vec![0.6, 0.8]), (2, vec![1.0, 0.0])]).unwrap();
        let view = remember_speaker(&d, M, 1, "2026-10-30T10:00:00Z".into()).unwrap();
        assert_eq!(view.voices.len(), 1);
        assert_eq!(
            (view.voices[0].has_sample, view.voices[0].meetings.len()),
            (false, 1)
        );
        // Повтор с той же встречи не добавляет второй центр.
        remember_speaker(&d, M, 1, "2026-10-30T10:01:00Z".into()).unwrap();
        let used = samples(&d, "e");
        assert_eq!(used.len(), 1);
        assert_eq!(used[0].name, "Анна");
        assert_eq!(used[0].embedding, vec![0.6, 0.8]);
    }

    #[test]
    fn voice_lists_its_archive_meetings_and_the_counter_drops_after_deletion() {
        let d = dir("meetings-of-voice");
        let later = "2026-10-31T09:00:00Z";
        for created_at in [M, later] {
            let mut t = transcript(&[Some("Анна")]);
            t.created_at = created_at.into();
            let id = crate::meetings::test_meeting(&d, &t);
            centers_file::save(&d, &id, &[(1, vec![0.6, 0.8])]).unwrap();
            remember_speaker(&d, created_at, 1, "2026-10-31T10:00:00Z".into()).unwrap();
        }
        // Центр встречи, которой в архиве нет: голос по нему узнаётся, но в списке её нет.
        update(&d, |voices| {
            voices[0].centers.push(MeetingCenter {
                meeting: "2026-01-01T00:00:00Z".into(),
                embedding: vec![1.0, 0.0],
                remembered_at: "2026-10-31T10:00:00Z".into(),
                voice_model: Some("e".into()),
            });
            Ok(())
        })
        .unwrap();
        let before = view(&d, &load(&d).unwrap());
        let ids: Vec<&str> = before.voices[0]
            .meetings
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(ids, [later, M]);
        assert!(!before.meetings_failed);

        crate::meetings::delete_meeting::delete(&d, later, &|dir| std::fs::remove_dir_all(dir))
            .unwrap();
        let voices = load(&d).unwrap();
        let after = view(&d, &voices);
        assert_eq!(after.voices[0].meetings.len(), 1);
        assert_eq!(after.voices[0].meetings[0].id, M);
        assert_eq!(voices[0].centers.len(), 2);
    }

    #[test]
    fn unreadable_index_leaves_voices_without_meetings_and_says_so() {
        let d = dir("meetings-of-voice-broken");
        let id = crate::meetings::test_meeting(&d, &transcript(&[Some("Анна")]));
        centers_file::save(&d, &id, &[(1, vec![0.6, 0.8])]).unwrap();
        remember_speaker(&d, M, 1, "2026-10-31T10:00:00Z".into()).unwrap();
        d.write(crate::meetings::INDEX_FILE, b"{").unwrap();
        let listed = view(&d, &load(&d).unwrap());
        assert!(listed.meetings_failed);
        assert_eq!(listed.voices.len(), 1);
        assert!(listed.voices[0].meetings.is_empty());
    }

    #[test]
    fn unnamed_unknown_or_centerless_speaker_cannot_be_remembered() {
        let d = dir("errors");
        let now = || "2026-10-30T10:00:00Z".to_string();
        assert_eq!(
            remember_speaker(&d, M, 1, "2026-11-07T12:00:00Z".into())
                .unwrap_err()
                .code,
            NO_TRANSCRIPT
        );
        let id = crate::meetings::test_meeting(&d, &transcript(&[Some("Анна"), None]));
        assert_eq!(
            remember_speaker(&d, M, 1, "2026-11-07T12:00:00Z".into())
                .unwrap_err()
                .code,
            NO_VOICE_CENTER
        );
        centers_file::save(&d, &id, &[(1, vec![1.0]), (2, vec![1.0])]).unwrap();
        assert_eq!(
            remember_speaker(&d, M, 2, now()).unwrap_err().code,
            SPEAKER_UNNAMED
        );
        assert_eq!(
            remember_speaker(&d, M, 9, now()).unwrap_err().code,
            UNKNOWN_SPEAKER
        );
        assert!(samples(&d, "e").is_empty());
    }

    #[test]
    fn samples_and_centers_of_another_voice_model_are_not_used() {
        let d = dir("models");
        let center = |meeting: &str, x: f32, model: Option<&str>| MeetingCenter {
            meeting: meeting.into(),
            embedding: vec![x],
            remembered_at: "2026-10-30T10:00:00Z".into(),
            voice_model: model.map(String::from),
        };
        let voice = |name: &str, model: Option<&str>, centers| VoiceRecord {
            name: name.into(),
            embedding: Some(vec![1.0]),
            voice_model: model.map(String::from),
            created_at: "2026-10-29T10:00:00Z".into(),
            centers,
        };
        store::update(&d, |v| {
            v.extend([
                voice(
                    "Анна",
                    Some("redimnet2-b6.onnx"),
                    vec![
                        center("m1", 0.5, Some("embedding.onnx")),
                        center("m2", 0.7, Some("redimnet2-b6.onnx")),
                    ],
                ),
                voice("Борис", None, vec![center("m1", 0.3, None)]),
                voice("Вера", Some("embedding.onnx"), Vec::new()),
            ]);
            Ok::<_, ()>(())
        })
        .unwrap()
        .unwrap();
        let used: Vec<(String, Vec<f32>)> = samples(&d, "redimnet2-b6.onnx")
            .into_iter()
            .map(|s| (s.name, s.embedding))
            .collect();
        assert_eq!(
            used,
            [("Анна".into(), vec![1.0]), ("Анна".into(), vec![0.7])]
        );
    }

    #[test]
    fn remembering_a_voice_of_a_deleted_meeting_leaves_the_library_unchanged() {
        let d = dir("after-delete");
        let id = crate::meetings::test_meeting(&d, &transcript(&[Some("Анна")]));
        centers_file::save(&d, &id, &[(1, vec![0.6, 0.8])]).unwrap();
        crate::meetings::delete_meeting::delete(&d, M, &|p| std::fs::remove_dir_all(p)).unwrap();
        assert_eq!(
            remember_speaker(&d, M, 1, "2026-11-07T12:00:00Z".into())
                .unwrap_err()
                .code,
            NO_TRANSCRIPT
        );
        assert!(store::load(&d).unwrap().is_empty());
    }

    #[test]
    fn remembered_center_is_marked_with_the_voice_model_of_its_transcript() {
        let d = dir("center-model");
        let id = crate::meetings::test_meeting(&d, &transcript(&[Some("Анна")]));
        centers_file::save(&d, &id, &[(1, vec![0.6, 0.8])]).unwrap();
        remember_speaker(&d, M, 1, "2026-10-30T10:00:00Z".into()).unwrap();
        let voices = store::load(&d).unwrap();
        assert_eq!(voices[0].centers[0].voice_model.as_deref(), Some("e"));
        assert!(samples(&d, "другая.onnx").is_empty());
    }
}
