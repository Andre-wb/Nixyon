/// Имена подстановок вида `{name}` в порядке появления, без повторов.
pub fn placeholders(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        let name = &after[..close];
        let valid = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if valid && !found.iter().any(|f| f == name) {
            found.push(name.to_string());
        }
        rest = &after[close + 1..];
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::placeholders;

    #[test]
    fn names_in_braces_are_found_once() {
        assert_eq!(placeholders("Спикер {n}"), ["n"]);
        assert_eq!(
            placeholders("{value} % из {total}, {value}"),
            ["total", "value"]
        );
        assert!(placeholders("без подстановок {} {Не}").is_empty());
    }
}
