use std::collections::BTreeMap;

use crate::check::check;
use crate::entries::parse;
use crate::locale_error::LocaleError;

/// Имя константы ключа в JS: `home.record` даёт `K.home_record`.
pub fn js_identifier(key: &str) -> String {
    key.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GenerateError {
    #[error("словари не прошли проверку:\n{0}")]
    Invalid(String),
    #[error("ключи {0} и {1} дают одно имя константы {2}")]
    Collision(String, String, String),
}

/// Модуль JS для интерфейса: константы ключей `K` и оба словаря. Словари встроены в
/// модуль, а не читаются запросом: политика безопасности страницы запросы не пускает.
pub fn generate(ru: &str, en: &str) -> Result<String, GenerateError> {
    let errors: Vec<LocaleError> = check(ru, en);
    if !errors.is_empty() {
        let text = errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        return Err(GenerateError::Invalid(text));
    }
    let to_map = |text: &str| -> BTreeMap<String, String> {
        parse(text)
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.key, e.value))
            .collect()
    };
    let (ru_map, en_map) = (to_map(ru), to_map(en));
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    for key in ru_map.keys() {
        let id = js_identifier(key);
        if let Some(other) = ids.insert(id.clone(), key.clone()) {
            return Err(GenerateError::Collision(other, key.clone(), id));
        }
    }
    let mut out = String::from(
        "// Создан генератором nixyon-locales из ui/locales при сборке. Не править руками.\n",
    );
    out.push_str("export const K = Object.freeze({\n");
    for (id, key) in &ids {
        out.push_str(&format!("  {id}: {},\n", json(key)));
    }
    out.push_str("});\n\nexport const DICTIONARIES = Object.freeze({\n");
    out.push_str(&format!("  ru: Object.freeze({}),\n", json(&ru_map)));
    out.push_str(&format!("  en: Object.freeze({}),\n", json(&en_map)));
    out.push_str("});\n");
    Ok(out)
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// Ключи, на которые ссылается интерфейс: `K.имя` в JS и `data-i18n*="ключ"` в HTML.
/// Имена констант переводятся обратно в ключи по словарю; неизвестные возвращаются как
/// есть, чтобы их можно было назвать в ошибке.
pub fn used_keys(sources: &[String], dictionary_keys: &[String]) -> Vec<String> {
    let by_id: BTreeMap<String, &String> = dictionary_keys
        .iter()
        .map(|k| (js_identifier(k), k))
        .collect();
    let mut found: Vec<String> = Vec::new();
    for text in sources {
        for (i, _) in text.match_indices("K.") {
            let before = text[..i].chars().next_back();
            if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
                continue;
            }
            let id: String = text[i + 2..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !id.is_empty() {
                found.push(by_id.get(&id).map_or(id, |k| (*k).clone()));
            }
        }
        for attr in ["data-i18n=\"", "data-i18n-label=\""] {
            for (i, _) in text.match_indices(attr) {
                let rest = &text[i + attr.len()..];
                if let Some(end) = rest.find('"') {
                    found.push(rest[..end].to_string());
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const RU: &str = r#"{"home.record": "Записать", "speaker.default": "Спикер {n}"}"#;
    const EN: &str = r#"{"home.record": "Record", "speaker.default": "Speaker {n}"}"#;

    #[test]
    fn module_has_constants_and_both_dictionaries() {
        let js = generate(RU, EN).unwrap();
        assert!(js.contains("  home_record: \"home.record\",\n"));
        assert!(js.contains("\"speaker.default\":\"Спикер {n}\""));
        assert!(js.contains("\"home.record\":\"Record\""));
    }

    #[test]
    fn broken_dictionaries_and_name_collisions_stop_generation() {
        assert!(matches!(
            generate(RU, r#"{"home.record": "Record"}"#),
            Err(GenerateError::Invalid(_))
        ));
        assert!(matches!(
            generate(r#"{"a.b": "1", "a_b": "2"}"#, r#"{"a.b": "1", "a_b": "2"}"#),
            Err(GenerateError::Collision(..))
        ));
    }

    #[test]
    fn keys_are_found_in_js_and_html_and_typos_stay_visible() {
        let dict = vec!["home.record".to_string(), "speaker.default".to_string()];
        let js = "t(K.home_record); t(K.home_recrod); x.K.no; JK.no".to_string();
        let html = r#"<b data-i18n="speaker.default"></b><i data-i18n-label="home.record"></i>"#
            .to_string();
        assert_eq!(
            used_keys(&[js, html], &dict),
            ["home.record", "home_recrod", "speaker.default"]
        );
    }
}
