use nixyon_core::BlockSegmenter;
use nixyon_ort::{Binarize, OrtError, OrtManifest, OrtRole, Sortformer};
use nixyon_sherpa::SherpaModels;

/// Нейросегментация для замера: pyannote из основного набора или Sortformer из набора ort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Segmenter {
    /// Модель роли segmentation основного набора.
    #[default]
    Pyannote,
    SortformerInt8,
    SortformerFp32,
}

impl Segmenter {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pyannote" => Some(Self::Pyannote),
            "sortformer-int8" => Some(Self::SortformerInt8),
            "sortformer-fp32" => Some(Self::SortformerFp32),
            _ => None,
        }
    }

    pub fn needs_ort(self) -> bool {
        self != Self::Pyannote
    }

    /// Модели с этой нейросегментацией; `threads`: потоки на вызов модели.
    pub fn apply(
        self,
        models: SherpaModels,
        ort: &OrtManifest,
        threads: usize,
    ) -> Result<SherpaModels, OrtError> {
        Ok(match self.load(ort, threads)? {
            None => models,
            Some(segmenter) => models.with_segmenter(segmenter),
        })
    }

    /// Нейросегментация из набора ort; None: pyannote из основного набора.
    pub fn load(
        self,
        ort: &OrtManifest,
        threads: usize,
    ) -> Result<Option<Box<dyn BlockSegmenter>>, OrtError> {
        let role = match self {
            Self::Pyannote => return Ok(None),
            Self::SortformerInt8 => OrtRole::SortformerInt8,
            Self::SortformerFp32 => OrtRole::SortformerFp32,
        };
        let sortformer = Sortformer::load(&ort.path(role)?, threads, Binarize::default())?;
        Ok(Some(Box::new(sortformer)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_parse_and_unknown_is_refused() {
        assert_eq!(Segmenter::parse("pyannote"), Some(Segmenter::Pyannote));
        assert_eq!(
            Segmenter::parse("sortformer-int8"),
            Some(Segmenter::SortformerInt8)
        );
        assert_eq!(
            Segmenter::parse("sortformer-fp32"),
            Some(Segmenter::SortformerFp32)
        );
        assert_eq!(Segmenter::parse("sortformer"), None);
    }

    #[test]
    fn only_pyannote_works_without_the_ort_set() {
        assert!(!Segmenter::Pyannote.needs_ort());
        assert!(Segmenter::SortformerInt8.needs_ort());
    }
}
