/// Горячие слова одной строкой для потока sherpa-onnx: фразы через «/». Косая черта
/// и переводы строк внутри фразы стали бы разделителями, поэтому заменяются пробелом;
/// пустые фразы выбрасываются.
pub fn hotwords_line(hotwords: &[String]) -> String {
    hotwords
        .iter()
        .map(|h| {
            h.replace(['/', '\n', '\r'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|h| !h.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(words: &[&str]) -> String {
        hotwords_line(&words.iter().map(|w| w.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn phrases_are_joined_with_slashes() {
        assert_eq!(line(&["Арчибальд", "Скайлс"]), "Арчибальд/Скайлс");
    }

    #[test]
    fn separators_inside_a_phrase_become_spaces() {
        assert_eq!(line(&["ООО «Рога/Копыта»"]), "ООО «Рога Копыта»");
        assert_eq!(line(&["два\nслова  тут"]), "два слова тут");
    }

    #[test]
    fn empty_phrases_are_dropped() {
        assert_eq!(line(&["", "  ", "Анна", "/"]), "Анна");
        assert_eq!(line(&[]), "");
    }
}
