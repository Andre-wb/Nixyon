/// Знаки валют, процента и градуса словами: модель пишет «2 ₽» и «100%», эталон
/// «два рубля» и «сто процентов». Слово потом сводится к одной форме (`number_units`).
pub(crate) fn spell_symbols(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for c in text.chars() {
        match c {
            '₽' => out.push_str(" руб "),
            '$' => out.push_str(" долл "),
            '€' => out.push_str(" евро "),
            '%' => out.push_str(" процент "),
            '°' => out.push_str(" градус "),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currency_percent_and_degree_signs_become_words() {
        assert_eq!(
            crate::normalize_words(&spell_symbols("2 ₽, 100%, 1°, за$200 и 5€")),
            [
                "2",
                "руб",
                "100",
                "процент",
                "1",
                "градус",
                "за",
                "долл",
                "200",
                "и",
                "5",
                "евро"
            ]
        );
    }
}
