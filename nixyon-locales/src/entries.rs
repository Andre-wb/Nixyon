use std::fmt;

use serde::de::{Deserializer, MapAccess, Visitor};

/// Пары ключ и значение в порядке файла, с повторами. Обычный разбор JSON в словарь
/// молча оставил бы последний повтор и потерял бы порядок, а оба нужны для проверки.
pub struct Entry {
    pub key: String,
    pub value: String,
    pub line: usize,
}

pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let pairs = serde_json::Deserializer::from_str(text)
        .deserialize_map(PairsVisitor)
        .map_err(|e| e.to_string())?;
    let mut seen: Vec<(String, usize)> = Vec::new();
    Ok(pairs
        .into_iter()
        .map(|(key, value)| {
            let nth = seen.iter().filter(|(k, _)| *k == key).count();
            let line = line_of(text, &key, nth);
            seen.push((key.clone(), line));
            Entry { key, value, line }
        })
        .collect())
}

struct PairsVisitor;

impl<'de> Visitor<'de> for PairsVisitor {
    type Value = Vec<(String, String)>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("объект, где все значения строки")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut pairs = Vec::new();
        while let Some((k, v)) = map.next_entry::<String, String>()? {
            pairs.push((k, v));
        }
        Ok(pairs)
    }
}

// Строка n-го появления ключа: ищется как "ключ" перед двоеточием.
fn line_of(text: &str, key: &str, nth: usize) -> usize {
    let needle = format!("{}:", serde_json::to_string(key).unwrap_or_default());
    let compact = |s: &str| s.replace(' ', "");
    text.lines()
        .enumerate()
        .filter(|(_, l)| compact(l).contains(&needle))
        .nth(nth)
        .map_or(0, |(i, _)| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_repeats_and_lines_are_kept() {
        let entries = parse("{\n  \"b\": \"1\",\n  \"a\": \"2\",\n  \"b\": \"3\"\n}").unwrap();
        let keys: Vec<(&str, &str, usize)> = entries
            .iter()
            .map(|e| (e.key.as_str(), e.value.as_str(), e.line))
            .collect();
        assert_eq!(keys, [("b", "1", 2), ("a", "2", 3), ("b", "3", 4)]);
    }

    #[test]
    fn non_string_values_and_broken_json_are_refused() {
        assert!(parse("{\"a\": 1}").is_err());
        assert!(parse("[\"a\"]").is_err());
        assert!(parse("{\"a\": \"x\"").is_err());
    }
}
