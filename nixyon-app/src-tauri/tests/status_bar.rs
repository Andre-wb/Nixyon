//! Окно рисуется под строкой состояния Android. При прокрутке текст не должен уходить под
//! часы и значки: сверху на высоту безопасной зоны лежит закреплённая полупрозрачная
//! затемняющая полоса, сквозь которую видно ambient-фон.

const STYLES: &str = include_str!("../../ui/styles.css");

/// Объявления правила верхнего уровня с точно таким селектором, без пробелов.
fn declarations(selector: &str) -> Option<Vec<String>> {
    let start = STYLES.find(&format!("\n{selector} {{"))?;
    let body = &STYLES[start..];
    let open = body.find('{')?;
    let close = body.find('}')?;
    Some(
        body[open + 1..close]
            .split(';')
            .map(|d| d.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|d| !d.is_empty())
            .collect(),
    )
}

#[test]
fn translucent_scrim_keeps_the_status_bar_legible_on_every_screen() {
    let rule = declarations("body::before").expect("нет правила body::before");
    for expected in [
        "position: fixed",
        "top: 0",
        "height: env(safe-area-inset-top)",
        "background: linear-gradient(180deg, rgba(16, 17, 18, 0.92), rgba(16, 17, 18, 0))",
        "pointer-events: none",
    ] {
        assert!(
            rule.iter().any(|d| d == expected),
            "в body::before нет «{expected}»: {rule:?}"
        );
    }
}
