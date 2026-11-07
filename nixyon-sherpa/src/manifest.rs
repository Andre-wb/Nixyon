use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nixyon_core::SpeechLanguage;
use serde::Deserialize;

use crate::{SherpaError, StreamModel};

/// Роли файлов в models.json; тот же набор читает Kotlin-плагин.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRole {
    Vad,
    AsrEncoder,
    AsrDecoder,
    AsrJoiner,
    AsrTokens,
    Embedding,
    /// Parakeet для английской речи.
    AsrEnEncoder,
    AsrEnDecoder,
    AsrEnJoiner,
    AsrEnTokens,
    /// Whisper tiny: определение языка записи.
    LidEncoder,
    LidDecoder,
}

impl ModelRole {
    pub const ALL: [ModelRole; 12] = [
        ModelRole::Vad,
        ModelRole::AsrEncoder,
        ModelRole::AsrDecoder,
        ModelRole::AsrJoiner,
        ModelRole::AsrTokens,
        ModelRole::Embedding,
        ModelRole::AsrEnEncoder,
        ModelRole::AsrEnDecoder,
        ModelRole::AsrEnJoiner,
        ModelRole::AsrEnTokens,
        ModelRole::LidEncoder,
        ModelRole::LidDecoder,
    ];

    /// Файлы распознавателя языка: кодировщик, декодер, объединитель и токены.
    pub fn asr(language: SpeechLanguage) -> [ModelRole; 4] {
        match language {
            SpeechLanguage::Ru => [
                ModelRole::AsrEncoder,
                ModelRole::AsrDecoder,
                ModelRole::AsrJoiner,
                ModelRole::AsrTokens,
            ],
            SpeechLanguage::En => [
                ModelRole::AsrEnEncoder,
                ModelRole::AsrEnDecoder,
                ModelRole::AsrEnJoiner,
                ModelRole::AsrEnTokens,
            ],
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ModelRole::Vad => "vad",
            ModelRole::AsrEncoder => "asr_encoder",
            ModelRole::AsrDecoder => "asr_decoder",
            ModelRole::AsrJoiner => "asr_joiner",
            ModelRole::AsrTokens => "asr_tokens",
            ModelRole::Embedding => "embedding",
            ModelRole::AsrEnEncoder => "asr_en_encoder",
            ModelRole::AsrEnDecoder => "asr_en_decoder",
            ModelRole::AsrEnJoiner => "asr_en_joiner",
            ModelRole::AsrEnTokens => "asr_en_tokens",
            ModelRole::LidEncoder => "lid_encoder",
            ModelRole::LidDecoder => "lid_decoder",
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

/// Необязательные роли: модель нейросегментации и словарь BPE GigaAM для горячих слов. В
/// основном наборе они есть, а в наборах для замера (models/variants) их может не быть.
const SEGMENTATION_KEY: &str = "segmentation";
const BPE_VOCAB_KEY: &str = "asr_bpe_vocab";
/// Необязательная потоковая модель для живого текста: четыре файла Zipformer вместе или
/// ни одного.
const LIVE_KEYS: [&str; 4] = ["live_encoder", "live_decoder", "live_joiner", "live_tokens"];
/// Необязательное распознавание звуков (речь, музыка): модель и список классов вместе
/// или ни одного.
const TAGGING_KEYS: [&str; 2] = ["tagging_model", "tagging_labels"];
/// Необязательный шумоподавитель GTCRN для второго VAD, который режет длинные куски; роль та
/// же, что в Kotlin-плагине.
const DENOISER_KEY: &str = "denoiser";

/// Набор моделей из models.json: версия и пути файлов по ролям.
#[derive(Debug, Clone)]
pub struct ModelManifest {
    version: String,
    dir: PathBuf,
    paths: BTreeMap<&'static str, String>,
    segmentation: Option<String>,
    bpe_vocab: Option<String>,
    live: Option<[String; 4]>,
    tagging: Option<[String; 2]>,
    denoiser: Option<String>,
}

impl ModelManifest {
    pub fn load(dir: &Path) -> Result<Self, SherpaError> {
        let path = dir.join("models.json");
        let text =
            std::fs::read_to_string(&path).map_err(|source| SherpaError::Read { path, source })?;
        Self::parse(dir, &text)
    }

    pub fn parse(dir: &Path, text: &str) -> Result<Self, SherpaError> {
        let raw: RawManifest =
            serde_json::from_str(text).map_err(|e| SherpaError::BadManifest(e.to_string()))?;
        let mut paths = BTreeMap::new();
        for role in ModelRole::ALL {
            let file = raw
                .files
                .get(role.key())
                .ok_or_else(|| SherpaError::BadManifest(format!("нет роли {}", role.key())))?;
            paths.insert(role.key(), checked(role.key(), file)?);
        }
        let segmentation = raw
            .files
            .get(SEGMENTATION_KEY)
            .map(|file| checked(SEGMENTATION_KEY, file))
            .transpose()?;
        let bpe_vocab = raw
            .files
            .get(BPE_VOCAB_KEY)
            .map(|file| checked(BPE_VOCAB_KEY, file))
            .transpose()?;
        let live = all_or_none(&raw.files, &LIVE_KEYS, "у живой модели есть не все файлы")?;
        let tagging = all_or_none(
            &raw.files,
            &TAGGING_KEYS,
            "у распознавания звуков есть не все файлы",
        )?;
        let denoiser = raw
            .files
            .get(DENOISER_KEY)
            .map(|file| checked(DENOISER_KEY, file))
            .transpose()?;
        Ok(Self {
            version: raw.version,
            dir: dir.to_path_buf(),
            paths,
            segmentation,
            bpe_vocab,
            live,
            tagging,
            denoiser,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn path(&self, role: ModelRole) -> String {
        // Все роли проверены в parse, поэтому ключ есть всегда.
        let name = &self.paths[role.key()];
        self.dir.join(name).to_string_lossy().into_owned()
    }

    /// Путь модели нейросегментации, если она есть в наборе.
    pub fn segmentation_path(&self) -> Option<String> {
        self.optional(self.segmentation.as_deref())
    }

    /// Путь словаря BPE GigaAM для горячих слов, если он есть в наборе. У Parakeet словаря
    /// нет: английская речь идёт без горячих слов (`Decoding::for_language`).
    pub fn bpe_vocab_path(&self) -> Option<String> {
        self.optional(self.bpe_vocab.as_deref())
    }

    /// Потоковая модель для живого текста, если она есть в наборе.
    pub fn live_model(&self) -> Option<StreamModel> {
        let [encoder, decoder, joiner, tokens] =
            self.live.as_ref()?.clone().map(|n| self.dir.join(n));
        Some(StreamModel::Zipformer {
            encoder,
            decoder,
            joiner,
            tokens,
        })
    }

    /// Модель распознавания звуков и список её классов, если они есть в наборе.
    pub fn tagging_paths(&self) -> Option<(String, String)> {
        let [model, labels] = self.tagging.as_ref()?;
        Some((self.path_of(model), self.path_of(labels)))
    }

    /// Путь шумоподавителя для второго VAD, если он есть в наборе.
    pub fn denoiser_path(&self) -> Option<String> {
        self.optional(self.denoiser.as_deref())
    }

    fn path_of(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }

    fn optional(&self, name: Option<&str>) -> Option<String> {
        name.map(|name| self.dir.join(name).to_string_lossy().into_owned())
    }
}

fn checked(key: &str, file: &RawFile) -> Result<String, SherpaError> {
    if is_plain_file_name(&file.path) {
        Ok(file.path.clone())
    } else {
        Err(SherpaError::BadManifest(format!(
            "недопустимый путь у роли {key}"
        )))
    }
}

// Файлы, которые нужны только вместе: часть без остальных значит, что набор собран
// неправильно.
fn all_or_none<const N: usize>(
    files: &BTreeMap<String, RawFile>,
    keys: &[&str; N],
    incomplete: &str,
) -> Result<Option<[String; N]>, SherpaError> {
    let found: Vec<String> = keys
        .iter()
        .filter_map(|key| files.get(*key).map(|file| checked(key, file)))
        .collect::<Result<_, _>>()?;
    match found.len() {
        0 => Ok(None),
        n if n == N => Ok(found.try_into().ok()),
        _ => Err(SherpaError::BadManifest(incomplete.into())),
    }
}

// Файл должен лежать прямо в папке моделей: без подпапок и выхода наверх.
fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\'])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_json(vad_path: &str) -> String {
        let mut files = String::new();
        for role in ModelRole::ALL {
            let path = if role == ModelRole::Vad {
                vad_path
            } else {
                "m.onnx"
            };
            if !files.is_empty() {
                files.push(',');
            }
            files.push_str(&format!(
                r#""{}": {{ "path": "{}", "size": 1, "sha256": "00" }}"#,
                role.key(),
                path
            ));
        }
        format!(r#"{{ "version": "v1", "files": {{ {files} }} }}"#)
    }

    #[test]
    fn parses_all_roles_and_joins_paths_with_models_dir() {
        let manifest = ModelManifest::parse(Path::new("/m"), &manifest_json("vad.onnx")).unwrap();
        assert_eq!(manifest.version(), "v1");
        assert_eq!(manifest.path(ModelRole::Vad), "/m/vad.onnx");
    }

    fn with_segmentation(path: &str) -> String {
        manifest_json("vad.onnx").replacen(
            r#""files": {"#,
            &format!(r#""files": {{ "segmentation": {{ "path": "{path}" }},"#),
            1,
        )
    }

    #[test]
    fn segmentation_model_is_optional() {
        let without = ModelManifest::parse(Path::new("/m"), &manifest_json("vad.onnx")).unwrap();
        assert_eq!(without.segmentation_path(), None);
        let with = ModelManifest::parse(Path::new("/m"), &with_segmentation("seg.onnx")).unwrap();
        assert_eq!(with.segmentation_path().as_deref(), Some("/m/seg.onnx"));
    }

    #[test]
    fn denoiser_is_optional() {
        let without = ModelManifest::parse(Path::new("/m"), &manifest_json("vad.onnx")).unwrap();
        assert_eq!(without.denoiser_path(), None);
        let text = manifest_json("vad.onnx").replacen(
            r#""files": {"#,
            r#""files": { "denoiser": { "path": "denoiser.onnx" },"#,
            1,
        );
        let with = ModelManifest::parse(Path::new("/m"), &text).unwrap();
        assert_eq!(with.denoiser_path().as_deref(), Some("/m/denoiser.onnx"));
        let outside = manifest_json("vad.onnx").replacen(
            r#""files": {"#,
            r#""files": { "denoiser": { "path": "../denoiser.onnx" },"#,
            1,
        );
        assert!(ModelManifest::parse(Path::new("/m"), &outside).is_err());
    }

    #[test]
    fn bpe_vocab_is_optional() {
        let without = ModelManifest::parse(Path::new("/m"), &manifest_json("vad.onnx")).unwrap();
        assert_eq!(without.bpe_vocab_path(), None);
        let text = manifest_json("vad.onnx").replacen(
            r#""files": {"#,
            r#""files": { "asr_bpe_vocab": { "path": "bpe.vocab" },"#,
            1,
        );
        let with = ModelManifest::parse(Path::new("/m"), &text).unwrap();
        assert_eq!(with.bpe_vocab_path().as_deref(), Some("/m/bpe.vocab"));
    }

    #[test]
    fn english_recognizer_and_language_id_are_required() {
        for role in [
            ModelRole::AsrEnEncoder,
            ModelRole::AsrEnTokens,
            ModelRole::LidEncoder,
            ModelRole::LidDecoder,
        ] {
            let files: Vec<String> = ModelRole::ALL
                .iter()
                .filter(|r| **r != role)
                .map(|r| format!(r#""{}": {{ "path": "m.onnx" }}"#, r.key()))
                .collect();
            let without = format!(
                r#"{{ "version": "v1", "files": {{ {} }} }}"#,
                files.join(",")
            );
            let err = ModelManifest::parse(Path::new("/m"), &without).unwrap_err();
            assert!(err.to_string().contains(role.key()), "{err}");
        }
    }

    #[test]
    fn each_language_has_its_own_recognizer_files() {
        let ru = ModelRole::asr(SpeechLanguage::Ru).map(ModelRole::key);
        let en = ModelRole::asr(SpeechLanguage::En).map(ModelRole::key);
        assert_eq!(
            ru,
            ["asr_encoder", "asr_decoder", "asr_joiner", "asr_tokens"]
        );
        assert_eq!(
            en,
            [
                "asr_en_encoder",
                "asr_en_decoder",
                "asr_en_joiner",
                "asr_en_tokens"
            ]
        );
    }

    fn with_live(keys: &[&str]) -> String {
        let files: String = keys
            .iter()
            .map(|key| format!(r#""{key}": {{ "path": "{key}.onnx" }},"#))
            .collect();
        manifest_json("vad.onnx").replacen(r#""files": {"#, &format!(r#""files": {{ {files}"#), 1)
    }

    #[test]
    fn live_model_is_optional_and_needs_all_four_files() {
        let without = ModelManifest::parse(Path::new("/m"), &manifest_json("vad.onnx")).unwrap();
        assert_eq!(without.live_model(), None);
        let with = ModelManifest::parse(Path::new("/m"), &with_live(&LIVE_KEYS)).unwrap();
        let live = with.live_model().unwrap();
        assert_eq!(live.name(), "zipformer");
        assert_eq!(live.files()[0], Path::new("/m/live_encoder.onnx"));
        assert_eq!(live.files()[3], Path::new("/m/live_tokens.onnx"));
        let err = ModelManifest::parse(Path::new("/m"), &with_live(&LIVE_KEYS[..3])).unwrap_err();
        assert!(err.to_string().contains("не все файлы"), "{err}");
    }

    #[test]
    fn audio_tagging_is_optional_and_needs_the_model_and_labels() {
        let without = ModelManifest::parse(Path::new("/m"), &manifest_json("vad.onnx")).unwrap();
        assert_eq!(without.tagging_paths(), None);
        let with = ModelManifest::parse(Path::new("/m"), &with_live(&TAGGING_KEYS)).unwrap();
        assert_eq!(
            with.tagging_paths(),
            Some((
                "/m/tagging_model.onnx".to_string(),
                "/m/tagging_labels.onnx".to_string()
            ))
        );
        let err =
            ModelManifest::parse(Path::new("/m"), &with_live(&TAGGING_KEYS[..1])).unwrap_err();
        assert!(err.to_string().contains("не все файлы"), "{err}");
    }

    #[test]
    fn segmentation_path_leaving_models_dir_is_rejected() {
        let err =
            ModelManifest::parse(Path::new("/m"), &with_segmentation("../seg.onnx")).unwrap_err();
        assert!(
            err.to_string()
                .contains("недопустимый путь у роли segmentation")
        );
    }

    #[test]
    fn rejects_manifest_without_a_role() {
        let text = r#"{ "version": "v1", "files": { "vad": { "path": "vad.onnx" } } }"#;
        let err = ModelManifest::parse(Path::new("/m"), text).unwrap_err();
        assert!(err.to_string().contains("нет роли"));
    }

    #[test]
    fn rejects_paths_leaving_models_dir() {
        // Обратная косая черта экранирована для JSON.
        for bad in [
            "../vad.onnx",
            "sub/vad.onnx",
            "..\\\\vad.onnx",
            ".hidden",
            "",
        ] {
            let err = ModelManifest::parse(Path::new("/m"), &manifest_json(bad)).unwrap_err();
            assert!(err.to_string().contains("недопустимый путь"), "{bad}");
        }
    }

    #[test]
    fn rejects_broken_json() {
        let err = ModelManifest::parse(Path::new("/m"), "{").unwrap_err();
        assert!(matches!(err, SherpaError::BadManifest(_)));
    }
}
