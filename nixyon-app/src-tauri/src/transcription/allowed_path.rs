use std::path::{Path, PathBuf};

/// Запись на обработку берётся только из папок приложения: путь приходит из
/// интерфейса, и без проверки им можно было бы указать на любой файл, доступный
/// приложению. Ссылки раскрываются до сравнения, поэтому выйти наружу через них нельзя.
pub fn resolve_input(path: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    let resolved = Path::new(path).canonicalize().ok()?;
    if !resolved.is_file() {
        return None;
    }
    roots
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| resolved.starts_with(root))
        .then_some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_files_inside_app_folders_are_accepted() {
        let base = std::env::temp_dir().join(format!("nixyon-allowed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let app = base.join("app");
        let other = base.join("other");
        std::fs::create_dir_all(app.join("rec")).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(app.join("rec/a.wav"), b"x").unwrap();
        std::fs::write(other.join("secret.wav"), b"x").unwrap();
        std::os::unix::fs::symlink(other.join("secret.wav"), app.join("link.wav")).unwrap();
        let roots = [app.clone()];

        let inside = app.join("rec/a.wav");
        assert_eq!(
            resolve_input(inside.to_str().unwrap(), &roots),
            Some(inside.canonicalize().unwrap())
        );
        let escape = format!("{}/rec/../../other/secret.wav", app.display());
        assert_eq!(resolve_input(&escape, &roots), None);
        assert_eq!(
            resolve_input(app.join("link.wav").to_str().unwrap(), &roots),
            None
        );
        assert_eq!(
            resolve_input(app.join("rec").to_str().unwrap(), &roots),
            None
        );
        assert_eq!(
            resolve_input(app.join("нет.wav").to_str().unwrap(), &roots),
            None
        );
    }
}
