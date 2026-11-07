use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::OrtError;

/// Роли файлов в models.json набора ort (`models/ort/`). Все роли необязательные:
/// набор для CI обходится без Sortformer fp32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrtRole {
    /// ReDimNet2-B6 в выгрузке soniqo, вход ровно 6 с.
    Redimnet2,
    /// Свои выгрузки весов PalabraAI с динамическим входом (scripts/export-redimnet2.py).
    Redimnet2B0,
    Redimnet2B1,
    Redimnet2B2,
    Redimnet2B3,
    /// B3, обученная на VoxBlink2, VoxCeleb2 и CN-Celeb2.
    Redimnet2B3Vb2,
    /// Те же веса, что у soniqo B6, но вход любой длины.
    Redimnet2B6Dyn,
    SortformerInt8,
    /// Веса int8 во внешнем файле: модель ищет его рядом с собой по имени.
    SortformerInt8Data,
    SortformerFp32,
    /// Пол и возраст по голосу: audEERING wav2vec2-large-robust, 6 слоёв.
    AgeGender,
    /// Эмоция реплики: GigaAM-Emo, веса MatMul в 8 бит. Лежит в основном наборе.
    Emotion,
}

impl OrtRole {
    pub fn key(self) -> &'static str {
        match self {
            OrtRole::Redimnet2 => "redimnet2",
            OrtRole::Redimnet2B0 => "redimnet2_b0",
            OrtRole::Redimnet2B1 => "redimnet2_b1",
            OrtRole::Redimnet2B2 => "redimnet2_b2",
            OrtRole::Redimnet2B3 => "redimnet2_b3",
            OrtRole::Redimnet2B3Vb2 => "redimnet2_b3_vb2",
            OrtRole::Redimnet2B6Dyn => "redimnet2_b6_dyn",
            OrtRole::SortformerInt8 => "sortformer_int8",
            OrtRole::SortformerInt8Data => "sortformer_int8_data",
            OrtRole::SortformerFp32 => "sortformer_fp32",
            OrtRole::AgeGender => "age_gender",
            OrtRole::Emotion => "emotion",
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    version: String,
    files: BTreeMap<String, RawFile>,
}

#[derive(Debug, Deserialize)]
struct RawFile {
    path: String,
}

/// Набор моделей ort: версия и пути файлов по ролям.
#[derive(Debug, Clone)]
pub struct OrtManifest {
    version: String,
    dir: PathBuf,
    paths: BTreeMap<String, String>,
}

impl OrtManifest {
    pub fn load(dir: &Path) -> Result<Self, OrtError> {
        let path = dir.join("models.json");
        let text =
            std::fs::read_to_string(&path).map_err(|source| OrtError::Read { path, source })?;
        Self::parse(dir, &text)
    }

    pub fn parse(dir: &Path, text: &str) -> Result<Self, OrtError> {
        let raw: RawManifest =
            serde_json::from_str(text).map_err(|e| OrtError::BadManifest(e.to_string()))?;
        let mut paths = BTreeMap::new();
        for (key, file) in raw.files {
            if !is_plain_file_name(&file.path) {
                return Err(OrtError::BadManifest(format!(
                    "недопустимый путь у роли {key}"
                )));
            }
            paths.insert(key, file.path);
        }
        Ok(Self {
            version: raw.version,
            dir: dir.to_path_buf(),
            paths,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// Путь файла роли; ошибка, если роли в наборе нет.
    pub fn path(&self, role: OrtRole) -> Result<PathBuf, OrtError> {
        self.paths
            .get(role.key())
            .map(|file| self.dir.join(file))
            .ok_or(OrtError::NoRole(role.key()))
    }
}

// Только имя файла внутри папки набора: без папок и выхода наверх.
fn is_plain_file_name(path: &str) -> bool {
    !path.is_empty() && path != "." && path != ".." && !path.contains('/') && !path.contains('\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = r#"{
        "version": "v1",
        "files": {
            "redimnet2": { "path": "redimnet2.onnx", "size": 1, "sha256": "00" },
            "sortformer_int8": { "path": "sortformer.onnx", "size": 1, "sha256": "00" }
        }
    }"#;

    #[test]
    fn roles_resolve_to_files_in_the_set_folder() {
        let m = OrtManifest::parse(Path::new("/m/ort"), TEXT).unwrap();
        assert_eq!(m.version(), "v1");
        assert_eq!(
            m.path(OrtRole::Redimnet2).unwrap(),
            Path::new("/m/ort/redimnet2.onnx")
        );
    }

    #[test]
    fn missing_role_is_an_error_naming_the_role() {
        let m = OrtManifest::parse(Path::new("/m"), TEXT).unwrap();
        let e = m.path(OrtRole::SortformerFp32).unwrap_err();
        assert!(e.to_string().contains("sortformer_fp32"));
    }

    #[test]
    fn path_outside_the_folder_is_refused() {
        let text = r#"{"version": "v", "files": {"redimnet2": {"path": "../x.onnx"}}}"#;
        assert!(OrtManifest::parse(Path::new("/m"), text).is_err());
    }
}
