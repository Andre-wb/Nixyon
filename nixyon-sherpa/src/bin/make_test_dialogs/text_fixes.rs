use std::collections::BTreeMap;

use serde::Deserialize;

/// Поправки опечаток в текстах PROMPTS: ключ `<сессия>/<файл>.wav`, замена слова целиком.
pub struct TextFixes(BTreeMap<String, Fix>);

#[derive(Deserialize)]
struct Fix {
    from: String,
    to: String,
}

impl TextFixes {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let map: BTreeMap<String, Fix> =
            serde_json::from_str(json).map_err(|e| format!("файл поправок текста: {e}"))?;
        if let Some((key, _)) = map
            .iter()
            .find(|(_, f)| f.from.trim().is_empty() || f.to.trim().is_empty())
        {
            return Err(format!("файл поправок текста: пустое слово у {key}"));
        }
        Ok(Self(map))
    }

    /// Текст реплики с поправкой. Если поправка есть, а слова в тексте нет, это ошибка:
    /// иначе поправка тихо перестала бы работать после смены текста.
    pub fn apply(&self, session: &str, file: &str, text: &str) -> Result<String, String> {
        let key = format!("{session}/{file}");
        let Some(fix) = self.0.get(&key) else {
            return Ok(text.to_string());
        };
        let words: Vec<&str> = text.split(' ').collect();
        if !words.contains(&fix.from.as_str()) {
            return Err(format!("поправка {key}: в тексте нет слова «{}»", fix.from));
        }
        Ok(words
            .into_iter()
            .map(|w| if w == fix.from { fix.to.as_str() } else { w })
            .collect::<Vec<_>>()
            .join(" "))
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_FIXES: &str = include_str!("../../../../scripts/data/voxforge-text-fixes.json");

    #[test]
    fn real_fixes_split_the_merged_za_nogu_in_the_snoop_clip() {
        let fixes = TextFixes::from_json(REAL_FIXES).unwrap();
        let prompt = "В руке он держал конец верёвочки к другому концу верёвочки была привязана заногу старая взлохмаченная ворона";
        let fixed = fixes
            .apply("snoop-20110506-gtp", "ru_0010.wav", prompt)
            .unwrap();
        assert!(fixed.contains("привязана за ногу старая"), "{fixed}");
        assert!(!fixed.contains("заногу"));
    }

    #[test]
    fn text_without_a_fix_stays_as_is() {
        let fixes = TextFixes::from_json(REAL_FIXES).unwrap();
        assert_eq!(fixes.apply("s", "a.wav", "как есть").unwrap(), "как есть");
    }

    #[test]
    fn fix_whose_word_is_gone_is_an_error() {
        let fixes =
            TextFixes::from_json(r#"{"s/a.wav": {"from": "заногу", "to": "за ногу"}}"#).unwrap();
        assert!(fixes.apply("s", "a.wav", "за ногу").is_err());
        assert!(fixes.apply("s", "a.wav", "взаногу").is_err());
    }

    #[test]
    fn empty_words_are_refused() {
        assert!(TextFixes::from_json(r#"{"s/a.wav": {"from": "", "to": "x"}}"#).is_err());
    }
}
