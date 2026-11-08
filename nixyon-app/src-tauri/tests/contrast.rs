//! Контраст цветов интерфейса. Nixyon использует один тёмный ambient-палитровый
//! режим на всех устройствах; старые значения data-theme не должны его менять.
//! Токены читаются из ui/styles.css: правила с переменными `--…` применяются по порядку,
//! если подходят их условия `@media` и селектор корня.

use std::collections::HashMap;

const STYLES: &str = include_str!("../../ui/styles.css");

/// Текст: не меньше 4,5:1 (WCAG AA для обычного текста).
const TEXT: f64 = 4.5;
/// Нетекстовое: кнопка записи, заливка кнопки и чипа рядом с фоном. Не меньше 3:1.
const NON_TEXT: f64 = 3.0;

#[derive(Clone, Copy, Debug)]
struct Mode {
    /// Выбор в настройках: None для «Авто», иначе атрибут data-theme.
    data_theme: Option<&'static str>,
    system_dark: bool,
    more_contrast: bool,
}

enum Node {
    Media(String, Vec<Node>),
    Rule(String, Vec<(String, String)>),
}

fn strip_comments(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        rest = rest[i + 2..]
            .find("*/")
            .map_or("", |j| &rest[i + 2 + j + 2..]);
    }
    out.push_str(rest);
    out
}

// Разбор правил верхнего уровня и внутри @media; остальные @-правила не нужны.
fn parse(css: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut rest = css.trim_start();
    while let Some(open) = rest.find('{') {
        let prelude = rest[..open].trim().to_string();
        let mut depth = 0;
        let mut close = open;
        for (i, c) in rest[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &rest[open + 1..close];
        if prelude.starts_with("@media") {
            nodes.push(Node::Media(prelude, parse(body)));
        } else if !prelude.starts_with('@') {
            let decls = body
                .split(';')
                .filter_map(|d| d.split_once(':'))
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                .filter(|(k, _)| k.starts_with("--"))
                .collect();
            nodes.push(Node::Rule(prelude, decls));
        }
        rest = rest[close + 1..].trim_start();
    }
    nodes
}

fn media_matches(prelude: &str, mode: Mode) -> bool {
    let condition = prelude.trim_start_matches("@media").trim();
    condition.split(',').any(|alternative| {
        alternative.split(" and ").all(|part| {
            let part = part.trim().trim_matches(|c| c == '(' || c == ')');
            match part.split_once(':').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("prefers-color-scheme", "dark")) => mode.system_dark,
                Some(("prefers-color-scheme", "light")) => !mode.system_dark,
                Some(("prefers-contrast", "more")) => mode.more_contrast,
                _ => false,
            }
        })
    })
}

// Селекторы корня документа: `:root`, `:root[data-theme="x"]`, `[data-theme="x"]`,
// `:root:not([data-theme="x"])`.
fn root_matches(selector: &str, mode: Mode) -> bool {
    selector.split(',').any(|one| {
        let mut s = one.trim();
        if let Some(r) = s.strip_prefix(":root") {
            s = r;
        } else if !s.starts_with("[data-theme") {
            return false;
        }
        let mut ok = true;
        while !s.is_empty() {
            if let Some(r) = s.strip_prefix(":not(") {
                let end = r.find(')').unwrap();
                ok &= !attr_matches(&r[..end], mode);
                s = &r[end + 1..];
            } else if s.starts_with('[') {
                let end = s.find(']').unwrap();
                ok &= attr_matches(&s[..=end], mode);
                s = &s[end + 1..];
            } else {
                return false;
            }
        }
        ok
    })
}

fn attr_matches(attr: &str, mode: Mode) -> bool {
    let value = attr
        .trim_matches(|c| c == '[' || c == ']')
        .strip_prefix("data-theme=")
        .map(|v| v.trim_matches('"'));
    value.is_some() && value == mode.data_theme
}

fn collect(nodes: &[Node], mode: Mode, tokens: &mut HashMap<String, String>) {
    for node in nodes {
        match node {
            Node::Media(prelude, inner) => {
                if media_matches(prelude, mode) {
                    collect(inner, mode, tokens);
                }
            }
            Node::Rule(selector, decls) => {
                if root_matches(selector, mode) {
                    for (k, v) in decls {
                        tokens.insert(k.clone(), v.clone());
                    }
                }
            }
        }
    }
}

fn luminance(hex: &str) -> f64 {
    let hex = hex.trim_start_matches('#');
    assert_eq!(hex.len(), 6, "ожидался цвет #rrggbb, а не {hex}");
    let channel = |i: usize| {
        let c = f64::from(u8::from_str_radix(&hex[i..i + 2], 16).unwrap()) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4)
}

fn contrast(a: &str, b: &str) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn modes() -> Vec<Mode> {
    let mut all = Vec::new();
    for data_theme in [None, Some("light"), Some("dark")] {
        for system_dark in [false, true] {
            for more_contrast in [false, true] {
                all.push(Mode {
                    data_theme,
                    system_dark,
                    more_contrast,
                });
            }
        }
    }
    all
}

#[test]
fn text_and_controls_have_enough_contrast_in_every_theme_mode() {
    let nodes = parse(&strip_comments(STYLES));
    // (передний план, фон, наименьший контраст)
    let pairs = [
        ("--label", "--bg", TEXT),
        ("--label", "--bg-elevated", TEXT),
        ("--label-secondary", "--bg", TEXT),
        ("--label-secondary", "--bg-elevated", TEXT),
        ("--accent-text", "--bg", TEXT),
        ("--accent-text", "--bg-elevated", TEXT),
        ("--danger", "--bg", TEXT),
        ("--danger", "--bg-elevated", TEXT),
        ("--warning-text", "--warning-bg", TEXT),
        ("--label", "--highlight", TEXT),
        ("--on-accent", "--accent-fill", TEXT),
        ("--on-danger", "--danger-fill", TEXT),
        ("--record", "--bg", NON_TEXT),
        ("--accent-fill", "--bg", NON_TEXT),
        ("--accent-fill", "--bg-elevated", NON_TEXT),
        ("--danger-fill", "--bg-elevated", NON_TEXT),
    ];
    let mut failures = Vec::new();
    for mode in modes() {
        let mut tokens = HashMap::new();
        collect(&nodes, mode, &mut tokens);
        for (fg, bg, need) in pairs {
            let (a, b) = (&tokens[fg], &tokens[bg]);
            let got = contrast(a, b);
            if got < need {
                failures.push(format!(
                    "{mode:?}: {fg} {a} на {bg} {b}: {got:.2}:1, нужно {need}:1"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_ambient_palette_is_the_same_in_every_system_mode() {
    let nodes = parse(&strip_comments(STYLES));
    let tokens = |data_theme, system_dark, more_contrast| {
        let mut tokens = HashMap::new();
        collect(
            &nodes,
            Mode {
                data_theme,
                system_dark,
                more_contrast,
            },
            &mut tokens,
        );
        tokens
    };
    for more in [false, true] {
        // Nixyon has one visual environment. The old theme setting must not fork
        // the palette between light, dark and auto system appearances.
        assert_eq!(tokens(Some("light"), true, more), tokens(None, false, more));
        assert_eq!(tokens(Some("dark"), false, more), tokens(None, true, more));
    }
}
