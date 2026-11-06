use std::path::{Path, PathBuf};

use sherpa_onnx::OnlineRecognizerConfig;

/// Потоковая русская модель для живого текста: пути её файлов.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamModel {
    /// `streaming-zipformer-small-ru-vosk-int8`: трансдьюсер, признаки на 16 кГц.
    Zipformer {
        encoder: PathBuf,
        decoder: PathBuf,
        joiner: PathBuf,
        tokens: PathBuf,
    },
    /// `streaming-t-one-russian`: CTC, признаки на 8 кГц.
    TOne { model: PathBuf, tokens: PathBuf },
}

// Тишина до и после записи, как в примерах sherpa-onnx для каждой модели: без хвоста
// последние слова не успевают выйти из модели.
const ZIPFORMER_PADDING_SEC: (f32, f32) = (0.0, 0.3);
const T_ONE_PADDING_SEC: (f32, f32) = (0.3, 0.66);
// Частота, на которой модель считает признаки; звук распознаватель передискретизирует сам.
const ZIPFORMER_FEATURE_RATE: i32 = 16_000;
const T_ONE_FEATURE_RATE: i32 = 8_000;

impl StreamModel {
    /// Zipformer из распакованного архива sherpa-onnx.
    pub fn zipformer_dir(dir: &Path) -> Self {
        Self::Zipformer {
            encoder: dir.join("encoder.int8.onnx"),
            decoder: dir.join("decoder.onnx"),
            joiner: dir.join("joiner.int8.onnx"),
            tokens: dir.join("tokens.txt"),
        }
    }

    /// T-one из распакованного архива sherpa-onnx.
    pub fn t_one_dir(dir: &Path) -> Self {
        Self::TOne {
            model: dir.join("model.onnx"),
            tokens: dir.join("tokens.txt"),
        }
    }

    /// Разбирает `zipformer=<папка архива>` или `t-one=<папка архива>`.
    pub fn parse(spec: &str) -> Option<Self> {
        let (kind, dir) = spec.split_once('=')?;
        match kind {
            "zipformer" => Some(Self::zipformer_dir(Path::new(dir))),
            "t-one" => Some(Self::t_one_dir(Path::new(dir))),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Zipformer { .. } => "zipformer",
            Self::TOne { .. } => "t-one",
        }
    }

    pub fn files(&self) -> Vec<&Path> {
        match self {
            Self::Zipformer {
                encoder,
                decoder,
                joiner,
                tokens,
            } => vec![encoder, decoder, joiner, tokens],
            Self::TOne { model, tokens } => vec![model, tokens],
        }
    }

    /// Секунды тишины до и после звука.
    pub fn padding_sec(&self) -> (f32, f32) {
        match self {
            Self::Zipformer { .. } => ZIPFORMER_PADDING_SEC,
            Self::TOne { .. } => T_ONE_PADDING_SEC,
        }
    }

    /// Настройки распознавателя: жадный поиск и границы фраз по тишине с правилами
    /// sherpa-onnx по умолчанию (нули заменяются на 2,4 с, 1,2 с и 20 с).
    pub fn config(&self, threads: i32) -> OnlineRecognizerConfig {
        let path = |p: &Path| Some(p.to_string_lossy().into_owned());
        let mut config = OnlineRecognizerConfig::default();
        match self {
            Self::Zipformer {
                encoder,
                decoder,
                joiner,
                tokens,
            } => {
                config.feat_config.sample_rate = ZIPFORMER_FEATURE_RATE;
                config.model_config.transducer.encoder = path(encoder);
                config.model_config.transducer.decoder = path(decoder);
                config.model_config.transducer.joiner = path(joiner);
                config.model_config.tokens = path(tokens);
            }
            Self::TOne { model, tokens } => {
                config.feat_config.sample_rate = T_ONE_FEATURE_RATE;
                config.model_config.t_one_ctc.model = path(model);
                config.model_config.tokens = path(tokens);
            }
        }
        config.model_config.num_threads = threads;
        config.model_config.provider = Some("cpu".into());
        config.decoding_method = Some("greedy_search".into());
        config.enable_endpoint = true;
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_is_parsed_from_kind_and_archive_folder() {
        assert_eq!(
            StreamModel::parse("t-one=t"),
            Some(StreamModel::t_one_dir(Path::new("t")))
        );
        assert_eq!(
            StreamModel::parse("zipformer=z").unwrap().files()[2],
            Path::new("z").join("joiner.int8.onnx")
        );
        assert_eq!(StreamModel::parse("vosk=z"), None);
        assert_eq!(StreamModel::parse("zipformer"), None);
    }

    #[test]
    fn each_model_gets_its_own_files_and_feature_rate() {
        let t_one = StreamModel::t_one_dir(Path::new("t")).config(1);
        assert_eq!(t_one.feat_config.sample_rate, 8_000);
        assert_eq!(
            t_one.model_config.t_one_ctc.model.as_deref(),
            Path::new("t").join("model.onnx").to_str()
        );
        assert!(t_one.model_config.transducer.encoder.is_none());

        let zipformer = StreamModel::zipformer_dir(Path::new("z")).config(2);
        assert_eq!(zipformer.feat_config.sample_rate, 16_000);
        assert_eq!(zipformer.model_config.num_threads, 2);
        assert!(zipformer.model_config.t_one_ctc.model.is_none());
        assert!(zipformer.enable_endpoint);
    }
}
