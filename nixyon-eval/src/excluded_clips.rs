use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Клипы с плохим эталоном, которые человек исключил из проверки: по наборам. Файл
/// пишет агент хаба: `{"<набор>": ["<id клипа>", ...]}`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExcludedClips(HashMap<String, HashSet<String>>);

impl ExcludedClips {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("исключённые клипы {}: {e}", path.display()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let sets: HashMap<String, Vec<String>> =
            serde_json::from_str(text).map_err(|e| format!("исключённые клипы: {e}"))?;
        Ok(Self(
            sets.into_iter()
                .map(|(set, ids)| (set, ids.into_iter().collect()))
                .collect(),
        ))
    }

    pub fn contains(&self, dataset: &str, clip: &str) -> bool {
        self.0.get(dataset).is_some_and(|ids| ids.contains(clip))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_are_excluded_only_in_their_own_set() {
        let excluded = ExcludedClips::parse(r#"{"cv": ["a", "b"], "radio": []}"#).unwrap();
        assert!(excluded.contains("cv", "a"));
        assert!(!excluded.contains("radio", "a"));
        assert!(!excluded.contains("calls", "a"));
        assert!(ExcludedClips::default() == ExcludedClips::parse("{}").unwrap());
        assert!(ExcludedClips::parse("[1]").is_err());
    }
}
