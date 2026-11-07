//! Список моделей из интерфейса: только известные ключи, без повторов.

/// Ключи в том же виде, что у `ModelPack` в Kotlin-плагине.
const KNOWN: [&str; 5] = ["speech", "live", "denoiser", "age", "llm"];

#[derive(Debug, PartialEq, Eq)]
pub struct UnknownPack;

pub fn parse_packs(packs: &[String]) -> Result<Vec<String>, UnknownPack> {
    let mut out: Vec<String> = Vec::new();
    for pack in packs {
        if !KNOWN.contains(&pack.as_str()) {
            return Err(UnknownPack);
        }
        if !out.contains(pack) {
            out.push(pack.clone());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|k| (*k).to_owned()).collect()
    }

    #[test]
    fn known_packs_pass_once_each_in_their_order() {
        assert_eq!(
            parse_packs(&owned(&["llm", "speech", "llm", "age"])),
            Ok(owned(&["llm", "speech", "age"]))
        );
        assert_eq!(parse_packs(&[]), Ok(Vec::new()));
    }

    #[test]
    fn an_unknown_or_oddly_written_pack_is_refused() {
        assert_eq!(
            parse_packs(&owned(&["speech", "tagging"])),
            Err(UnknownPack)
        );
        assert_eq!(parse_packs(&owned(&["LLM"])), Err(UnknownPack));
        assert_eq!(parse_packs(&owned(&["../speech"])), Err(UnknownPack));
    }
}
