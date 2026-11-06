/// Текст для XML: спецсимволы заменены сущностями, а символы, которых XML 1.0 не
/// допускает вовсе (управляющие, кроме табуляции и переносов, U+FFFE и U+FFFF),
/// выброшены: иначе Word откажется открыть файл.
pub fn xml_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if c < ' ' || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::xml_text;

    #[test]
    fn markup_is_escaped_and_forbidden_characters_are_dropped() {
        assert_eq!(
            xml_text("a < b & \"c\" > d"),
            "a &lt; b &amp; &quot;c&quot; &gt; d"
        );
        assert_eq!(xml_text("да\u{0}\u{1b}\u{FFFF}нет"), "данет");
        assert_eq!(xml_text("Привет, мир 😀"), "Привет, мир 😀");
    }
}
