use std::io::BufRead;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{DatasetKind, ReferenceSource};

const INFO_FILE: &str = "dataset.json";
const CLIPS_FILE: &str = "clips.jsonl";

/// Описание набора из `<корень>/<id>/dataset.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetInfo {
    pub id: String,
    pub title: String,
    pub kind: DatasetKind,
    pub has_text: bool,
    /// Кто писал тексты; у старых описаний без поля неизвестно.
    #[serde(default)]
    pub reference: ReferenceSource,
    pub license: String,
    pub clips: usize,
    pub hours: f64,
}

/// Клип из `clips.jsonl`: путь к звуку от корня наборов, текст, если он есть, длительность,
/// число голосов, если оно известно, и путь к разметке спикеров (`<имя>.truth.json`),
/// если она есть.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexClip {
    pub id: String,
    pub audio: String,
    pub text: Option<String>,
    pub duration: f64,
    pub speakers: Option<u32>,
    #[serde(default)]
    pub truth: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    pub info: DatasetInfo,
    pub clips: Vec<IndexClip>,
}

#[derive(Debug, thiserror::Error)]
pub enum DatasetError {
    #[error("имя набора «{0}»: только строчные латинские буквы, цифры и _")]
    BadId(String),
    #[error("набор {0}: {1}")]
    Io(String, std::io::Error),
    #[error("набор {0}: описание или строка {1} не читается: {2}")]
    Parse(String, usize, serde_json::Error),
    #[error("набор {0}: в описании другое имя ({1})")]
    IdMismatch(String, String),
    #[error("набор {0}, клип {1}: путь к звуку или разметке должен идти вниз от корня наборов")]
    BadAudioPath(String, String),
    #[error("набор {0}: клипов нет")]
    Empty(String),
}

pub fn is_valid_dataset_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Описания всех наборов в корне, по имени. Папки без описания или с битым описанием
/// пропускаются: в корне лежат и сами записи.
pub fn list_datasets(root: &Path) -> Vec<DatasetInfo> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<DatasetInfo> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| is_valid_dataset_id(name))
        .filter_map(|name| read_info(root, &name).ok())
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

pub fn load_dataset(root: &Path, id: &str) -> Result<Dataset, DatasetError> {
    let info = read_info(root, id)?;
    let file = std::fs::File::open(root.join(id).join(CLIPS_FILE))
        .map_err(|e| DatasetError::Io(id.to_owned(), e))?;
    let mut clips = Vec::new();
    for (n, line) in std::io::BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| DatasetError::Io(id.to_owned(), e))?;
        if line.trim().is_empty() {
            continue;
        }
        let clip: IndexClip = serde_json::from_str(&line)
            .map_err(|e| DatasetError::Parse(id.to_owned(), n + 1, e))?;
        if !is_safe_relative(&clip.audio)
            || clip.truth.as_deref().is_some_and(|t| !is_safe_relative(t))
        {
            return Err(DatasetError::BadAudioPath(id.to_owned(), clip.id));
        }
        clips.push(clip);
    }
    if clips.is_empty() {
        return Err(DatasetError::Empty(id.to_owned()));
    }
    Ok(Dataset { info, clips })
}

/// Полный путь к звуку клипа. Путь в индексе уже проверен при загрузке.
pub fn audio_path(root: &Path, clip: &IndexClip) -> PathBuf {
    root.join(&clip.audio)
}

fn read_info(root: &Path, id: &str) -> Result<DatasetInfo, DatasetError> {
    if !is_valid_dataset_id(id) {
        return Err(DatasetError::BadId(id.to_owned()));
    }
    let text = std::fs::read_to_string(root.join(id).join(INFO_FILE))
        .map_err(|e| DatasetError::Io(id.to_owned(), e))?;
    let info: DatasetInfo =
        serde_json::from_str(&text).map_err(|e| DatasetError::Parse(id.to_owned(), 0, e))?;
    if info.id != id {
        return Err(DatasetError::IdMismatch(id.to_owned(), info.id));
    }
    Ok(info)
}

// Индекс пишут скрипты подготовки наборов, но путь всё равно не должен уводить из корня.
fn is_safe_relative(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Root(PathBuf);

    impl Root {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nixyon-datasets-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn dataset(&self, id: &str, info_id: &str, clips: &[&str]) {
            let dir = self.0.join(id);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(INFO_FILE),
                format!(
                    r#"{{"id":"{info_id}","title":"Набор","kind":"single","has_text":true,"license":"CC0","clips":{},"hours":0.1}}"#,
                    clips.len()
                ),
            )
            .unwrap();
            std::fs::write(dir.join(CLIPS_FILE), clips.join("\n")).unwrap();
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const CLIP: &str = r#"{"id":"a","audio":"src/a.wav","text":"раз","duration":1.5,"speakers":1}"#;

    #[test]
    fn datasets_are_listed_by_id_and_folders_without_description_are_skipped() {
        let root = Root::new("list");
        root.dataset("b_set", "b_set", &[CLIP]);
        root.dataset("a_set", "a_set", &[CLIP]);
        std::fs::create_dir_all(root.0.join("sources")).unwrap();
        let ids: Vec<String> = list_datasets(&root.0).into_iter().map(|d| d.id).collect();
        assert_eq!(ids, ["a_set", "b_set"]);
    }

    #[test]
    fn a_dataset_is_loaded_with_its_clips() {
        let root = Root::new("load");
        let no_text =
            r#"{"id":"b","audio":"src/b.opus","text":null,"duration":2.0,"speakers":null}"#;
        root.dataset("cv", "cv", &[CLIP, no_text]);
        let set = load_dataset(&root.0, "cv").unwrap();
        assert_eq!(set.info.kind, DatasetKind::Single);
        assert_eq!(set.info.reference, ReferenceSource::Unknown, "поля нет");
        assert_eq!(set.clips.len(), 2);
        assert_eq!(set.clips[1].text, None);
        assert_eq!(audio_path(&root.0, &set.clips[0]), root.0.join("src/a.wav"));
    }

    #[test]
    fn who_wrote_the_texts_is_read_from_the_description() {
        let root = Root::new("reference");
        let dir = root.0.join("calls");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(INFO_FILE),
            r#"{"id":"calls","title":"Звонки","kind":"single","has_text":true,"reference":"human","license":"CC0","clips":1,"hours":0.1}"#,
        )
        .unwrap();
        std::fs::write(dir.join(CLIPS_FILE), CLIP).unwrap();
        let set = load_dataset(&root.0, "calls").unwrap();
        assert_eq!(set.info.reference, ReferenceSource::Human);
    }

    #[test]
    fn audio_paths_leaving_the_root_are_refused() {
        let root = Root::new("paths");
        for (n, audio) in ["../x.wav", "/etc/passwd", "src/../../x.wav", ""]
            .iter()
            .enumerate()
        {
            let id = format!("set{n}");
            let clip = format!(
                r#"{{"id":"a","audio":"{audio}","text":"раз","duration":1.0,"speakers":1}}"#
            );
            root.dataset(&id, &id, &[&clip]);
            assert!(
                matches!(
                    load_dataset(&root.0, &id),
                    Err(DatasetError::BadAudioPath(..))
                ),
                "{audio}"
            );
        }
    }

    #[test]
    fn a_truth_path_is_read_and_must_stay_under_the_root() {
        let root = Root::new("truth");
        let with_truth = r#"{"id":"d","audio":"src/d.wav","text":"раз","duration":9.0,"speakers":2,"truth":"src/d.truth.json"}"#;
        root.dataset("talk", "talk", &[with_truth, CLIP]);
        let set = load_dataset(&root.0, "talk").unwrap();
        assert_eq!(set.clips[0].truth.as_deref(), Some("src/d.truth.json"));
        assert_eq!(set.clips[1].truth, None);
        let escaping = r#"{"id":"d","audio":"src/d.wav","text":"раз","duration":9.0,"speakers":2,"truth":"../d.truth.json"}"#;
        root.dataset("bad", "bad", &[escaping]);
        assert!(matches!(
            load_dataset(&root.0, "bad"),
            Err(DatasetError::BadAudioPath(..))
        ));
    }

    #[test]
    fn bad_ids_and_a_description_of_another_set_are_refused() {
        let root = Root::new("ids");
        root.dataset("cv", "other", &[CLIP]);
        assert!(matches!(
            load_dataset(&root.0, "cv"),
            Err(DatasetError::IdMismatch(..))
        ));
        assert!(matches!(
            load_dataset(&root.0, "../cv"),
            Err(DatasetError::BadId(_))
        ));
        assert!(matches!(
            load_dataset(&root.0, "CV"),
            Err(DatasetError::BadId(_))
        ));
        assert!(list_datasets(&root.0).is_empty());
    }
}
