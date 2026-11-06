use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use crate::child_age::child_by_age;
use crate::{LabeledVoiceClip, VoiceGender};

/// Части набора: модель не обучается, для замера берутся обе.
const SPLITS: [&str; 2] = ["train", "test"];

#[derive(Debug, thiserror::Error)]
pub enum SpeechoceanError {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("{0}, строка {1}: ждали два поля через пробел или табуляцию")]
    BadLine(PathBuf, usize),
    #[error("{0}: возраст {1} не число")]
    BadAge(PathBuf, String),
    #[error("{0}: недопустимый путь звука {1}")]
    BadAudioPath(PathBuf, String),
    #[error("{0}: у клипа {1} нет говорящего")]
    NoSpeaker(PathBuf, String),
}

/// Клипы набора speechocean762 (OpenSLR 101) из папки, где лежат `train/` и `test/` в
/// формате Kaldi. Пол из `spk2gender`, возраст из `spk2age`. Порядок по id клипа.
pub fn read_speechocean(dir: &Path) -> Result<Vec<LabeledVoiceClip>, SpeechoceanError> {
    let mut clips = Vec::new();
    for split in SPLITS {
        clips.extend(read_split(dir, split)?);
    }
    Ok(sorted(clips))
}

/// Клипы одной части набора (`train` или `test`), порядок по id клипа. Говорящие в частях
/// разные, поэтому по частям можно обучать и проверять.
pub fn read_speechocean_split(
    dir: &Path,
    split: &str,
) -> Result<Vec<LabeledVoiceClip>, SpeechoceanError> {
    Ok(sorted(read_split(dir, split)?))
}

fn sorted(mut clips: Vec<(String, LabeledVoiceClip)>) -> Vec<LabeledVoiceClip> {
    clips.sort_by(|a, b| a.0.cmp(&b.0));
    clips.into_iter().map(|(_, clip)| clip).collect()
}

fn read_split(
    dir: &Path,
    split: &str,
) -> Result<Vec<(String, LabeledVoiceClip)>, SpeechoceanError> {
    let part = dir.join(split);
    let genders = read_pairs(&part.join("spk2gender"))?;
    let ages = read_pairs(&part.join("spk2age"))?;
    let speakers = read_pairs(&part.join("utt2spk"))?;
    let scp = part.join("wav.scp");
    let mut clips = Vec::new();
    for (utt, audio) in read_pairs(&scp)? {
        if !is_inside(&audio) {
            return Err(SpeechoceanError::BadAudioPath(scp, audio));
        }
        let speaker = speakers
            .get(&utt)
            .ok_or_else(|| SpeechoceanError::NoSpeaker(scp.clone(), utt.clone()))?;
        let child = match ages.get(speaker) {
            Some(age) => child_by_age(
                age.parse()
                    .map_err(|_| SpeechoceanError::BadAge(part.join("spk2age"), age.clone()))?,
            ),
            None => None,
        };
        clips.push((
            utt,
            LabeledVoiceClip {
                speaker: format!("{split}/{speaker}"),
                audio: dir.join(audio),
                gender: genders.get(speaker).and_then(|g| parse_gender(g)),
                child,
            },
        ));
    }
    Ok(clips)
}

fn parse_gender(value: &str) -> Option<VoiceGender> {
    match value {
        "m" => Some(VoiceGender::Male),
        "f" => Some(VoiceGender::Female),
        _ => None,
    }
}

fn read_pairs(path: &Path) -> Result<HashMap<String, String>, SpeechoceanError> {
    let text = std::fs::read_to_string(path).map_err(|e| SpeechoceanError::Io(path.into(), e))?;
    let mut pairs = HashMap::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        match (fields.next(), fields.next(), fields.next()) {
            (Some(key), Some(value), None) => {
                pairs.insert(key.to_string(), value.to_string());
            }
            _ => return Err(SpeechoceanError::BadLine(path.into(), i + 1)),
        }
    }
    Ok(pairs)
}

// Путь звука относительный и не выходит из папки набора.
fn is_inside(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nixyon-so762-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            for split in SPLITS {
                std::fs::create_dir_all(path.join(split)).unwrap();
            }
            Self(path)
        }

        fn split(&self, split: &str, gender: &str, age: &str, utt2spk: &str, scp: &str) {
            let part = self.0.join(split);
            std::fs::write(part.join("spk2gender"), gender).unwrap();
            std::fs::write(part.join("spk2age"), age).unwrap();
            std::fs::write(part.join("utt2spk"), utt2spk).unwrap();
            std::fs::write(part.join("wav.scp"), scp).unwrap();
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn clips_of_both_splits_get_gender_and_child_by_age() {
        let dir = Dir::new("ok");
        dir.split(
            "train",
            "0001\tm\n0002\tf\n",
            "0001\t8\n0002\t30\n",
            "u2 0002\nu1 0001\n",
            "u2\tW/2.WAV\nu1\tW/1.WAV\n",
        );
        dir.split(
            "test",
            "0003\tf\n",
            "0003\t15\n",
            "u3 0003\n",
            "u3\tW/3.WAV\n",
        );
        let clips = read_speechocean(&dir.0).unwrap();
        assert_eq!(clips.len(), 3);
        assert_eq!(clips[0].audio, dir.0.join("W/1.WAV"));
        assert_eq!(clips[0].speaker, "train/0001");
        assert_eq!(clips[0].gender, Some(VoiceGender::Male));
        assert_eq!(clips[0].child, Some(true));
        assert_eq!(clips[1].child, Some(false));
        assert_eq!(clips[2].speaker, "test/0003");
        assert_eq!(clips[2].child, None);
    }

    #[test]
    fn one_split_is_read_on_its_own() {
        let dir = Dir::new("split");
        dir.split("train", "0001 m\n", "0001 8\n", "u1 0001\n", "u1 W/1.WAV\n");
        dir.split("test", "0002 f\n", "0002 30\n", "u2 0002\n", "u2 W/2.WAV\n");
        let clips = read_speechocean_split(&dir.0, "test").unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].speaker, "test/0002");
        assert_eq!(clips[0].child, Some(false));
    }

    #[test]
    fn an_audio_path_leaving_the_folder_is_refused() {
        let dir = Dir::new("path");
        dir.split("train", "", "", "u1 0001\n", "u1 ../x.WAV\n");
        dir.split("test", "", "", "", "");
        assert!(matches!(
            read_speechocean(&dir.0),
            Err(SpeechoceanError::BadAudioPath(_, _))
        ));
    }

    #[test]
    fn a_clip_without_a_speaker_is_an_error() {
        let dir = Dir::new("speaker");
        dir.split("train", "", "", "", "u1 W/1.WAV\n");
        dir.split("test", "", "", "", "");
        assert!(matches!(
            read_speechocean(&dir.0),
            Err(SpeechoceanError::NoSpeaker(_, _))
        ));
    }

    #[test]
    fn a_line_with_three_fields_is_an_error() {
        let dir = Dir::new("line");
        dir.split("train", "0001 m x\n", "", "", "");
        dir.split("test", "", "", "", "");
        assert!(matches!(
            read_speechocean(&dir.0),
            Err(SpeechoceanError::BadLine(_, 1))
        ));
    }

    #[test]
    fn a_non_numeric_age_is_an_error() {
        let dir = Dir::new("age");
        dir.split("train", "", "0001 old\n", "u1 0001\n", "u1 W/1.WAV\n");
        dir.split("test", "", "", "", "");
        assert!(matches!(
            read_speechocean(&dir.0),
            Err(SpeechoceanError::BadAge(_, _))
        ));
    }
}
