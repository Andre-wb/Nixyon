use std::path::{Path, PathBuf};

fn main() {
    generate_i18n();
    tauri_build::build()
}

// Модуль ключей и словарей для интерфейса строится из ui/locales при каждой сборке.
// Сборка падает, если словари не прошли проверку или код интерфейса ссылается на
// ключ, которого нет в словаре: опечатка в ключе видна до запуска приложения.
fn generate_i18n() {
    let ui = Path::new("../ui");
    let ru_path = ui.join("locales/ru.json");
    let en_path = ui.join("locales/en.json");
    let ru = read(&ru_path);
    let en = read(&en_path);
    println!("cargo:rerun-if-changed={}", ru_path.display());
    println!("cargo:rerun-if-changed={}", en_path.display());

    let module = nixyon_locales::generate(&ru, &en).unwrap_or_else(|e| panic!("{e}"));
    let target = ui.join("generated/i18n.js");
    if std::fs::read_to_string(&target).ok().as_deref() != Some(module.as_str()) {
        std::fs::create_dir_all(ui.join("generated")).expect("папка ui/generated не создаётся");
        std::fs::write(&target, module).expect("ui/generated/i18n.js не записывается");
    }

    let files = ui_sources(ui);
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    let sources: Vec<String> = files.iter().map(|f| read(f)).collect();
    let keys: Vec<String> = serde_keys(&ru);
    let missing: Vec<String> = nixyon_locales::used_keys(&sources, &keys)
        .into_iter()
        .filter(|k| !keys.contains(k))
        .collect();
    assert!(
        missing.is_empty(),
        "в коде интерфейса есть ключи, которых нет в ui/locales/ru.json: {}",
        missing.join(", ")
    );
}

// Код интерфейса: все .js и .html, кроме сгенерированного.
fn ui_sources(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n != "generated" && n != "locales")
            {
                out.extend(ui_sources(&path));
            }
        } else if path.extension().is_some_and(|e| e == "js" || e == "html") {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn serde_keys(json: &str) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for line in json.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('"')
            && let Some(end) = rest.find("\":")
        {
            keys.push(rest[..end].to_string());
        }
    }
    keys
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{} не читается: {e}", path.display()))
}
