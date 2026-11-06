use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::child_age::child_by_age;
use crate::{LabeledVoiceClip, VoiceGender};

/// Части набора, которые читаются, если их папка есть.
const SPLITS: [&str; 3] = ["train", "dev", "test"];

#[derive(Debug, thiserror::Error)]
pub enum ChildMandarinError {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("{0}: разметка не читается: {1}")]
    BadLabel(PathBuf, String),
    #[error("в {0} нет ни одной части: train, dev, test")]
    NoSplit(PathBuf),
}

#[derive(Deserialize)]
struct Label {
    speaker_id: String,
    age: u32,
    gender: String,
}

/// Клипы набора ChildMandarin (BAAI) из папки с распакованными частями: в
/// `<часть>/<говорящий>/` у каждого `.wav` рядом `.json` с говорящим, возрастом и полом.
/// Порядок по пути звука.
pub fn read_childmandarin(dir: &Path) -> Result<Vec<LabeledVoiceClip>, ChildMandarinError> {
    let mut clips = Vec::new();
    let mut found = false;
    for split in SPLITS {
        let part = dir.join(split);
        if !part.is_dir() {
            continue;
        }
        found = true;
        for speaker_dir in sorted_entries(&part)? {
            if !speaker_dir.is_dir() {
                continue;
            }
            for label in sorted_entries(&speaker_dir)? {
                if label.extension().is_some_and(|e| e == "json") {
                    clips.push(read_clip(split, &label)?);
                }
            }
        }
    }
    if !found {
        return Err(ChildMandarinError::NoSplit(dir.into()));
    }
    Ok(clips)
}

fn read_clip(split: &str, label: &Path) -> Result<LabeledVoiceClip, ChildMandarinError> {
    let text =
        std::fs::read_to_string(label).map_err(|e| ChildMandarinError::Io(label.into(), e))?;
    let parsed: Label = serde_json::from_str(&text)
        .map_err(|e| ChildMandarinError::BadLabel(label.into(), e.to_string()))?;
    Ok(LabeledVoiceClip {
        speaker: format!("{split}/{}", parsed.speaker_id),
        audio: label.with_extension("wav"),
        gender: match parsed.gender.as_str() {
            "M" => Some(VoiceGender::Male),
            "F" => Some(VoiceGender::Female),
            _ => None,
        },
        child: child_by_age(parsed.age),
    })
}

fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>, ChildMandarinError> {
    let entries = std::fs::read_dir(dir).map_err(|e| ChildMandarinError::Io(dir.into(), e))?;
    let mut paths = Vec::new();
    for entry in entries {
        paths.push(
            entry
                .map_err(|e| ChildMandarinError::Io(dir.into(), e))?
                .path(),
        );
    }
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nixyon-cm-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn label(&self, rel: &str, json: &str) {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, json).unwrap();
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn each_label_gives_a_clip_with_speaker_gender_and_age() {
        let dir = Dir::new("ok");
        dir.label(
            "test/002/002_5_M_001.json",
            r#"{"id":"x","speaker_id":"002","age":5,"gender":"M"}"#,
        );
        dir.label(
            "dev/007/007_3_F_001.json",
            r#"{"id":"y","speaker_id":"007","age":3,"gender":"F"}"#,
        );
        let clips = read_childmandarin(&dir.0).unwrap();
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].speaker, "dev/007");
        assert_eq!(clips[0].gender, Some(VoiceGender::Female));
        assert_eq!(clips[1].audio, dir.0.join("test/002/002_5_M_001.wav"));
        assert_eq!(clips[1].child, Some(true));
    }

    #[test]
    fn a_broken_label_is_an_error() {
        let dir = Dir::new("bad");
        dir.label("test/002/a.json", r#"{"speaker_id":"002"}"#);
        assert!(matches!(
            read_childmandarin(&dir.0),
            Err(ChildMandarinError::BadLabel(_, _))
        ));
    }

    #[test]
    fn a_folder_without_splits_is_an_error() {
        let dir = Dir::new("empty");
        assert!(matches!(
            read_childmandarin(&dir.0),
            Err(ChildMandarinError::NoSplit(_))
        ));
    }
}
