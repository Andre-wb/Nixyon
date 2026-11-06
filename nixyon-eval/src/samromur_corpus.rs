use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::child_age::child_by_age_text;
use crate::{LabeledVoiceClip, VoiceGender};

const METADATA: &str = "metadata.tsv";
/// Глубина поиска `metadata.tsv` и звука от папки набора: архивы кладут всё в одну папку
/// верхнего уровня.
const MAX_DEPTH: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum SamromurError {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("в {0} нет {METADATA}")]
    NoMetadata(PathBuf),
    #[error("{0}: нет столбца {1}")]
    NoColumn(PathBuf, &'static str),
    #[error("{0}, строка {1}: число полей не совпадает с шапкой")]
    BadRow(PathBuf, usize),
}

/// Клип Samrómur с частью набора из столбца `status` (train, dev, test).
#[derive(Debug, Clone, PartialEq)]
pub struct SamromurClip {
    pub split: String,
    pub clip: LabeledVoiceClip,
}

/// Клипы набора Samrómur (взрослый 21.05 или детский Children 21.09, OpenSLR 112 и 117):
/// разметка в `metadata.tsv`, звук `.flac` где-то в папках говорящих. Возраст годами или
/// диапазоном из анкеты. Клипы, у которых звука нет, пропускаются. Порядок как в таблице.
pub fn read_samromur(dir: &Path) -> Result<Vec<SamromurClip>, SamromurError> {
    let metadata =
        find_metadata(dir, MAX_DEPTH)?.ok_or_else(|| SamromurError::NoMetadata(dir.into()))?;
    let mut audio = HashMap::new();
    index_audio(metadata.parent().unwrap_or(dir), MAX_DEPTH, &mut audio)?;
    let text =
        std::fs::read_to_string(&metadata).map_err(|e| SamromurError::Io(metadata.clone(), e))?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split('\t').collect();
    let column = |name: &'static str| {
        header
            .iter()
            .position(|h| *h == name)
            .ok_or_else(|| SamromurError::NoColumn(metadata.clone(), name))
    };
    let (speaker, file, gender, age, status) = (
        column("speaker_id")?,
        column("filename")?,
        column("gender")?,
        column("age")?,
        column("status")?,
    );
    let name = dir
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let mut clips = Vec::new();
    for (i, line) in lines.enumerate() {
        if line.is_empty() {
            continue;
        }
        let row: Vec<&str> = line.split('\t').collect();
        if row.len() != header.len() {
            return Err(SamromurError::BadRow(metadata.clone(), i + 2));
        }
        let Some(path) = audio.get(row[file]) else {
            continue;
        };
        clips.push(SamromurClip {
            split: row[status].to_string(),
            clip: LabeledVoiceClip {
                speaker: format!("{name}/{}", row[speaker]),
                audio: path.clone(),
                gender: match row[gender] {
                    "male" => Some(VoiceGender::Male),
                    "female" => Some(VoiceGender::Female),
                    _ => None,
                },
                child: child_by_age_text(row[age]),
            },
        });
    }
    Ok(clips)
}

fn entries(dir: &Path) -> Result<Vec<PathBuf>, SamromurError> {
    let read = std::fs::read_dir(dir).map_err(|e| SamromurError::Io(dir.into(), e))?;
    let mut paths = Vec::new();
    for entry in read {
        paths.push(entry.map_err(|e| SamromurError::Io(dir.into(), e))?.path());
    }
    paths.sort();
    Ok(paths)
}

fn find_metadata(dir: &Path, depth: usize) -> Result<Option<PathBuf>, SamromurError> {
    let candidate = dir.join(METADATA);
    if candidate.is_file() {
        return Ok(Some(candidate));
    }
    if depth == 0 {
        return Ok(None);
    }
    for path in entries(dir)? {
        if is_real_dir(&path)
            && let Some(found) = find_metadata(&path, depth - 1)?
        {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

// Ссылки не обходятся: набор распакован из чужого архива.
fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

fn index_audio(
    dir: &Path,
    depth: usize,
    audio: &mut HashMap<String, PathBuf>,
) -> Result<(), SamromurError> {
    for path in entries(dir)? {
        let meta =
            std::fs::symlink_metadata(&path).map_err(|e| SamromurError::Io(path.clone(), e))?;
        if meta.is_dir() && depth > 0 {
            index_audio(&path, depth - 1, audio)?;
        } else if meta.is_file()
            && path.extension().is_some_and(|e| e == "flac")
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            audio.insert(name.to_string(), path.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nixyon-samromur-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, rel: &str, text: &str) {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const HEADER: &str = "id\tspeaker_id\tfilename\tgender\tage\tstatus\n";

    #[test]
    fn clips_get_split_gender_and_child_by_age_and_their_audio_is_found() {
        let dir = Dir::new("ok");
        dir.write(
            "kids/metadata.tsv",
            &format!(
                "{HEADER}1\t006\t006-1.flac\tmale\t10\ttrain\n2\t007\t007-2.flac\tfemale\t13-17\ttest\n3\t008\t008-3.flac\tother\t25\tdev\n4\t009\t009-4.flac\tmale\t9\ttrain\n"
            ),
        );
        dir.write("kids/data_train/006/006-1.flac", "");
        dir.write("kids/data_test/007/007-2.flac", "");
        dir.write("kids/data_dev/008/008-3.flac", "");
        let clips = read_samromur(&dir.0).unwrap();
        assert_eq!(clips.len(), 3);
        assert_eq!(clips[0].split, "train");
        assert_eq!(
            clips[0].clip.audio,
            dir.0.join("kids/data_train/006/006-1.flac")
        );
        assert_eq!(clips[0].clip.gender, Some(VoiceGender::Male));
        assert_eq!(clips[0].clip.child, Some(true));
        assert!(clips[0].clip.speaker.ends_with("/006"));
        assert_eq!(clips[1].clip.child, None);
        assert_eq!(
            (clips[2].clip.gender, clips[2].clip.child),
            (None, Some(false))
        );
    }

    #[test]
    fn a_folder_without_metadata_or_a_broken_row_is_an_error() {
        let dir = Dir::new("bad");
        assert!(matches!(
            read_samromur(&dir.0),
            Err(SamromurError::NoMetadata(_))
        ));
        dir.write("metadata.tsv", &format!("{HEADER}1\t006\n"));
        assert!(matches!(
            read_samromur(&dir.0),
            Err(SamromurError::BadRow(_, 2))
        ));
    }

    #[test]
    fn a_missing_column_is_an_error() {
        let dir = Dir::new("column");
        dir.write("metadata.tsv", "speaker_id\tfilename\n");
        assert!(matches!(
            read_samromur(&dir.0),
            Err(SamromurError::NoColumn(_, "gender"))
        ));
    }
}
