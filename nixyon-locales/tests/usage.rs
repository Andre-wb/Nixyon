//! Ключи словаря и ключи, на которые ссылается код, совпадают в обе стороны: в коде нет
//! ключей, которых нет в словаре, а в словаре нет ключей, которыми никто не пользуется.
//! Пользуются ими интерфейс (ui/**/*.js, ui/**/*.html) и экспорт (export_labels.rs).

use std::path::{Path, PathBuf};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

fn ui_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n != "generated" && n != "locales")
            {
                ui_sources(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "js" || e == "html") {
            out.push(path);
        }
    }
}

fn dictionary_keys() -> Vec<String> {
    let text = std::fs::read_to_string(format!("{ROOT}/nixyon-app/ui/locales/ru.json")).unwrap();
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    map.keys().cloned().collect()
}

#[test]
fn every_key_in_code_is_in_the_dictionary_and_every_dictionary_key_is_used() {
    let keys = dictionary_keys();
    let mut files = Vec::new();
    ui_sources(Path::new(&format!("{ROOT}/nixyon-app/ui")), &mut files);
    assert!(!files.is_empty(), "код интерфейса не найден");
    let sources: Vec<String> = files
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap())
        .collect();
    let mut used = nixyon_locales::used_keys(&sources, &keys);
    let export =
        std::fs::read_to_string(format!("{ROOT}/nixyon-locales/src/export_labels.rs")).unwrap();
    used.extend(
        keys.iter()
            .filter(|k| export.contains(&format!("\"{k}\"")))
            .cloned(),
    );
    used.sort();
    used.dedup();
    let missing: Vec<&String> = used.iter().filter(|k| !keys.contains(k)).collect();
    let unused: Vec<&String> = keys.iter().filter(|k| !used.contains(k)).collect();
    assert!(
        missing.is_empty(),
        "в коде есть ключи, которых нет в словаре: {missing:?}"
    );
    assert!(
        unused.is_empty(),
        "в словаре есть ключи, которыми никто не пользуется: {unused:?}"
    );
}
