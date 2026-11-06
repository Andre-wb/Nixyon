use super::voice_record::VoiceRecord;

/// Наибольшая длина имени в знаках, как у переименования спикера.
const MAX_NAME_CHARS: usize = 64;

/// Имя голоса без пробелов по краям. None: пустое, длиннее предела или с управляющими
/// знаками.
pub fn clean_name(name: &str) -> Option<String> {
    let name = name.trim();
    let ok = !name.is_empty()
        && name.chars().count() <= MAX_NAME_CHARS
        && !name.chars().any(char::is_control);
    ok.then(|| name.to_string())
}

/// У голоса с таким именем уже есть образец. Имя сравнивается без учёта регистра:
/// «анна» и «Анна» спутать легко.
pub fn has_sample(voices: &[VoiceRecord], name: &str) -> bool {
    let name = name.to_lowercase();
    voices
        .iter()
        .any(|v| v.embedding.is_some() && v.name.to_lowercase() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_trimmed_and_checked() {
        assert_eq!(clean_name("  Анна "), Some("Анна".to_string()));
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name("а\nб"), None);
        assert_eq!(
            clean_name(&"я".repeat(64)).map(|n| n.chars().count()),
            Some(64)
        );
        assert_eq!(clean_name(&"я".repeat(65)), None);
    }

    #[test]
    fn sample_is_found_by_name_regardless_of_case() {
        let voice = |name: &str, embedding| VoiceRecord {
            name: name.into(),
            embedding,
            voice_model: None,
            created_at: String::new(),
            centers: Vec::new(),
        };
        let voices = [voice("Анна", Some(vec![1.0])), voice("Борис", None)];
        assert!(has_sample(&voices, "анна"));
        assert!(has_sample(&voices, "АННА"));
        assert!(!has_sample(&voices, "Борис"));
        assert!(!has_sample(&voices, "Вера"));
    }
}
