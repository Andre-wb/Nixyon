use nixyon_core::VoiceEmbedder;
use nixyon_ort::{OrtError, OrtManifest, OrtRole, Redimnet2};
use nixyon_sherpa::SherpaModels;

/// Имена ReDimNet2 во флаге `--voice-model` и роли их файлов в наборе ort.
const REDIMNET2: [(&str, OrtRole); 7] = [
    ("redimnet2", OrtRole::Redimnet2),
    ("redimnet2-b0", OrtRole::Redimnet2B0),
    ("redimnet2-b1", OrtRole::Redimnet2B1),
    ("redimnet2-b2", OrtRole::Redimnet2B2),
    ("redimnet2-b3", OrtRole::Redimnet2B3),
    ("redimnet2-b3-vb2", OrtRole::Redimnet2B3Vb2),
    ("redimnet2-b6-dyn", OrtRole::Redimnet2B6Dyn),
];

/// Модель голоса для замера: из основного набора (sherpa-onnx) или ReDimNet2 из набора ort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceModel {
    /// Модель роли embedding основного набора.
    #[default]
    Campplus,
    /// ReDimNet2 из файла этой роли набора ort.
    Redimnet2(OrtRole),
}

impl VoiceModel {
    pub fn parse(value: &str) -> Option<Self> {
        if value == "campplus" {
            return Some(Self::Campplus);
        }
        REDIMNET2
            .iter()
            .find(|(name, _)| *name == value)
            .map(|(_, role)| Self::Redimnet2(*role))
    }

    /// Имя, под которым модель задаётся во флаге `--voice-model`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Campplus => "campplus",
            Self::Redimnet2(role) => REDIMNET2
                .iter()
                .find(|(_, r)| *r == role)
                .map_or("redimnet2", |(name, _)| name),
        }
    }

    pub fn needs_ort(self) -> bool {
        self != Self::Campplus
    }

    /// Модели с этой моделью голоса; `threads`: потоки на вызов модели.
    /// Модели, где модель голоса заменена этой; `jobs` сессий, чтобы эмбеддинги шли
    /// параллельно.
    pub fn apply(
        self,
        models: SherpaModels,
        ort: &OrtManifest,
        threads: usize,
        jobs: usize,
    ) -> Result<SherpaModels, OrtError> {
        Ok(match self.load(ort, threads, jobs)? {
            None => models,
            Some(voice) => models.with_voice(voice),
        })
    }

    /// Модель голоса из набора ort; None: CAM++ из основного набора.
    pub fn load(
        self,
        ort: &OrtManifest,
        threads: usize,
        jobs: usize,
    ) -> Result<Option<Box<dyn VoiceEmbedder>>, OrtError> {
        Ok(match self {
            Self::Campplus => None,
            Self::Redimnet2(role) => Some(Box::new(Redimnet2::load_pool(
                &ort.path(role)?,
                threads,
                jobs,
            )?)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_parse_and_unknown_is_refused() {
        assert_eq!(VoiceModel::parse("campplus"), Some(VoiceModel::Campplus));
        assert_eq!(
            VoiceModel::parse("redimnet2"),
            Some(VoiceModel::Redimnet2(OrtRole::Redimnet2))
        );
        assert_eq!(VoiceModel::parse("titanet"), None);
    }

    #[test]
    fn each_light_and_dynamic_redimnet_name_points_to_its_own_role() {
        let cases = [
            ("redimnet2-b0", OrtRole::Redimnet2B0),
            ("redimnet2-b1", OrtRole::Redimnet2B1),
            ("redimnet2-b2", OrtRole::Redimnet2B2),
            ("redimnet2-b3", OrtRole::Redimnet2B3),
            ("redimnet2-b3-vb2", OrtRole::Redimnet2B3Vb2),
            ("redimnet2-b6-dyn", OrtRole::Redimnet2B6Dyn),
        ];
        for (name, role) in cases {
            assert_eq!(VoiceModel::parse(name), Some(VoiceModel::Redimnet2(role)));
        }
        assert_eq!(VoiceModel::parse("redimnet2-b4"), None);
    }

    #[test]
    fn the_name_parses_back_into_the_same_model() {
        let names = ["campplus", "redimnet2", "redimnet2-b2", "redimnet2-b6-dyn"];
        for name in names {
            assert_eq!(VoiceModel::parse(name).map(VoiceModel::name), Some(name));
        }
    }

    #[test]
    fn only_the_base_model_works_without_the_ort_set() {
        assert!(!VoiceModel::Campplus.needs_ort());
        assert!(VoiceModel::Redimnet2(OrtRole::Redimnet2B0).needs_ort());
    }
}
