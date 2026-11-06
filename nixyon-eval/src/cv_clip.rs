use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Клип Common Voice: звук `<id>.mp3` и разметка `<id>.json` с эталонным текстом.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CvClip {
    pub id: String,
    pub text: String,
    /// Длительность в секундах из разметки набора.
    pub duration: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CvClipFiles {
    pub clip: CvClip,
    pub mp3: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum CvClipError {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("{0}: разметка не читается: {1}")]
    Json(PathBuf, serde_json::Error),
    #[error("{0}: id в разметке {1} не совпадает с именем файла")]
    IdMismatch(PathBuf, String),
    #[error("{0}: длительность должна быть числом больше нуля")]
    BadDuration(PathBuf),
    #[error("{0}: нет звука к разметке")]
    MissingMp3(PathBuf),
    #[error("в {0} нет клипов: ожидались пары <id>.mp3 и <id>.json")]
    NoClips(PathBuf),
}

/// Все клипы папки по возрастанию id. Пары проверяются до распознавания: битая разметка
/// видна сразу, а не через час работы.
pub fn list_cv_clips(dir: &Path) -> Result<Vec<CvClipFiles>, CvClipError> {
    let entries = std::fs::read_dir(dir).map_err(|e| CvClipError::Io(dir.to_path_buf(), e))?;
    let mut clips = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| CvClipError::Io(dir.to_path_buf(), e))?
            .path();
        if path.extension().is_some_and(|e| e == "json") {
            clips.push(read_clip(&path)?);
        }
    }
    if clips.is_empty() {
        return Err(CvClipError::NoClips(dir.to_path_buf()));
    }
    clips.sort_by(|a, b| a.clip.id.cmp(&b.clip.id));
    Ok(clips)
}

fn read_clip(json: &Path) -> Result<CvClipFiles, CvClipError> {
    let text = std::fs::read_to_string(json).map_err(|e| CvClipError::Io(json.to_path_buf(), e))?;
    let clip: CvClip =
        serde_json::from_str(&text).map_err(|e| CvClipError::Json(json.to_path_buf(), e))?;
    let stem = json.file_stem().map(|s| s.to_string_lossy());
    if stem.as_deref() != Some(clip.id.as_str()) {
        return Err(CvClipError::IdMismatch(json.to_path_buf(), clip.id));
    }
    if !(clip.duration.is_finite() && clip.duration > 0.0) {
        return Err(CvClipError::BadDuration(json.to_path_buf()));
    }
    let mp3 = json.with_extension("mp3");
    if !mp3.is_file() {
        return Err(CvClipError::MissingMp3(json.to_path_buf()));
    }
    Ok(CvClipFiles { clip, mp3 })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nixyon-cv-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn clip(&self, id: &str, json: &str, with_mp3: bool) {
            std::fs::write(self.0.join(format!("{id}.json")), json).unwrap();
            if with_mp3 {
                std::fs::write(self.0.join(format!("{id}.mp3")), b"").unwrap();
            }
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn clips_are_listed_by_id_with_their_mp3() {
        let dir = Dir::new("ok");
        dir.clip("b", r#"{"id":"b","text":"два","duration":2.0}"#, true);
        dir.clip("a", r#"{"id":"a","text":"раз","duration":1.5}"#, true);
        let clips = list_cv_clips(&dir.0).unwrap();
        let ids: Vec<&str> = clips.iter().map(|c| c.clip.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(clips[0].mp3, dir.0.join("a.mp3"));
        assert_eq!(clips[0].clip.text, "раз");
    }

    #[test]
    fn a_json_without_its_mp3_is_an_error() {
        let dir = Dir::new("no-mp3");
        dir.clip("a", r#"{"id":"a","text":"раз","duration":1.5}"#, false);
        assert!(matches!(
            list_cv_clips(&dir.0),
            Err(CvClipError::MissingMp3(_))
        ));
    }

    #[test]
    fn an_id_that_differs_from_the_file_name_is_an_error() {
        let dir = Dir::new("id");
        dir.clip("a", r#"{"id":"z","text":"раз","duration":1.5}"#, true);
        assert!(matches!(
            list_cv_clips(&dir.0),
            Err(CvClipError::IdMismatch(_, _))
        ));
    }

    #[test]
    fn a_zero_duration_is_an_error() {
        let dir = Dir::new("duration");
        dir.clip("a", r#"{"id":"a","text":"раз","duration":0}"#, true);
        assert!(matches!(
            list_cv_clips(&dir.0),
            Err(CvClipError::BadDuration(_))
        ));
    }

    #[test]
    fn an_empty_folder_is_an_error() {
        let dir = Dir::new("empty");
        assert!(matches!(
            list_cv_clips(&dir.0),
            Err(CvClipError::NoClips(_))
        ));
    }

    #[test]
    fn the_bundled_clip_is_readable() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        let clips = list_cv_clips(&dir).unwrap();
        let clip = clips
            .iter()
            .find(|c| c.clip.id == "common_voice_ru_38615634")
            .unwrap();
        assert_eq!(clip.clip.text, "Никогда со мной этого не бывало.");
    }
}
