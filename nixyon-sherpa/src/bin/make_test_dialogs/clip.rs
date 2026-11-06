use std::path::{Path, PathBuf};

use crate::known_bounds::KnownBounds;
use crate::text_fixes::TextFixes;

pub struct Clip {
    pub person: String,
    pub path: PathBuf,
    pub session: Option<String>,
    pub text: Option<String>,
    /// Проверенные границы речи в секундах от начала клипа; None: по громкости.
    pub speech_sec: Option<(f64, f64)>,
}

/// Тестовые записи sherpa-onnx для real3: три человека, текста нет.
pub fn real3_clips(sherpa: &Path) -> Vec<Clip> {
    [
        "fangjun-sr-1",
        "leijun-sr-1",
        "liudehua-sr-1",
        "fangjun-sr-2",
        "leijun-test-sr-1",
        "liudehua-sr-2",
        "fangjun-test-sr-1",
        "leijun-sr-2",
        "liudehua-test-sr-1",
        "fangjun-sr-3",
        "leijun-test-sr-2",
        "liudehua-test-sr-2",
    ]
    .iter()
    .map(|name| Clip {
        person: name.split('-').next().unwrap_or(name).to_string(),
        path: sherpa.join(format!("{name}.wav")),
        session: None,
        text: None,
        speech_sec: None,
    })
    .collect()
}

/// Какие клипы сессии брать: по порядку имён файлов, два подряд начиная с `first`.
/// `known_bounds`: границы речи из файла проверенных границ, иначе по громкости.
#[derive(Clone, Copy)]
pub struct Pick {
    pub first: usize,
    pub known_bounds: bool,
}

/// Клипы диалогов: первые два в каждой сессии, границы проверены.
pub const DIALOG_CLIPS: Pick = Pick {
    first: 0,
    known_bounds: true,
};

// По две записи из каждой из двух сессий человека: голос тот же, микрофон и комната разные.
pub fn voxforge_clips(
    voxforge: &Path,
    person: &str,
    pick: Pick,
    bounds: &KnownBounds,
    fixes: &TextFixes,
) -> Result<Vec<Clip>, Box<dyn std::error::Error>> {
    let mut sessions: Vec<PathBuf> = std::fs::read_dir(voxforge)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{person}-")))
        })
        .collect();
    sessions.sort();
    if sessions.len() < 2 {
        return Err(format!("у {person} меньше двух сессий").into());
    }
    let mut clips = Vec::new();
    for session in &sessions[..2] {
        let mut wavs: Vec<PathBuf> = std::fs::read_dir(session.join("wav"))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "wav"))
            .collect();
        wavs.sort();
        if wavs.len() < pick.first + 2 {
            return Err(
                format!("в {} меньше {} записей", session.display(), pick.first + 2).into(),
            );
        }
        let prompts = std::fs::read_to_string(session.join("etc/PROMPTS"))?;
        let session_name = session
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or("у сессии нет имени")?;
        for wav in wavs.into_iter().skip(pick.first).take(2) {
            let text = prompt_text(&prompts, &session_name, &wav)
                .ok_or_else(|| format!("нет текста для {} в PROMPTS", wav.display()))?;
            let file = wav
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .ok_or("у клипа нет имени")?;
            let text = fixes.apply(&session_name, &file, &text)?;
            let speech_sec = if pick.known_bounds {
                Some(
                    bounds
                        .seconds(&session_name, &file)
                        .ok_or_else(|| format!("нет границ речи для {session_name}/{file}"))?,
                )
            } else {
                None
            };
            clips.push(Clip {
                person: person.to_string(),
                path: wav,
                session: Some(session_name.clone()),
                text: Some(text),
                speech_sec,
            });
        }
    }
    Ok(clips)
}

// Строка PROMPTS: `<сессия>/mfc/<имя файла без .wav> <текст>`.
fn prompt_text(prompts: &str, session: &str, wav: &Path) -> Option<String> {
    let key = format!("{session}/mfc/{}", wav.file_stem()?.to_string_lossy());
    prompts.lines().find_map(|line| {
        let (id, text) = line.split_once(' ')?;
        (id == key).then(|| text.trim().to_string())
    })
}
