use std::path::Path;

/// Сколько прошлых прогонов хранится: итоги прогона весят сотни КБ.
pub const KEEP_RUNS: usize = 100;
const SUMMARY: &str = "summary.json";

/// Имя прогона в истории: время конца (секунды Unix, 10 цифр, поэтому имена
/// сортируются по времени) и, если известен, коммит.
pub fn run_name(finished_at: u64, commit: Option<&str>) -> String {
    match commit {
        Some(commit) => format!("{finished_at:010}-{commit}"),
        None => format!("{finished_at:010}"),
    }
}

/// Коммит в имени прогона: 7-40 строчных шестнадцатеричных знаков.
pub fn is_commit(text: &str) -> bool {
    (7..=40).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn is_run_name(name: &str) -> bool {
    let (time, commit) = match name.split_once('-') {
        Some((time, commit)) => (time, Some(commit)),
        None => (name, None),
    };
    time.len() == 10 && time.bytes().all(|b| b.is_ascii_digit()) && commit.is_none_or(is_commit)
}

/// Кладёт копию итогов прогона в `<история>/<имя>/summary.json` (через папку `.part`,
/// чтобы недописанный прогон не попал в историю) и оставляет только `keep` последних.
pub fn archive_run(summary: &Path, history: &Path, name: &str, keep: usize) -> std::io::Result<()> {
    if !is_run_name(name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("имя прогона {name}"),
        ));
    }
    std::fs::create_dir_all(history)?;
    let part = history.join(format!("{name}.part"));
    let _ = std::fs::remove_dir_all(&part);
    std::fs::create_dir(&part)?;
    std::fs::copy(summary, part.join(SUMMARY))?;
    let done = history.join(name);
    let _ = std::fs::remove_dir_all(&done);
    std::fs::rename(&part, &done)?;
    prune(history, keep)
}

fn prune(history: &Path, keep: usize) -> std::io::Result<()> {
    let mut runs: Vec<String> = std::fs::read_dir(history)?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| is_run_name(name))
        .collect();
    runs.sort_unstable_by(|a, b| b.cmp(a));
    for old in runs.iter().skip(keep) {
        std::fs::remove_dir_all(history.join(old))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nixyon-history-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn names_sort_by_time_and_carry_the_commit() {
        assert_eq!(
            run_name(1_761_905_400, Some("91ecf11")),
            "1761905400-91ecf11"
        );
        assert_eq!(run_name(42, None), "0000000042");
        assert!(is_run_name("1761905400-91ecf11"));
        assert!(is_run_name("1761905400"));
        assert!(!is_run_name("1761905400-XYZ"));
        assert!(!is_run_name("../1761905400"));
        assert!(!is_run_name("176190540"));
        assert!(!is_commit("abc"));
    }

    #[test]
    fn a_run_is_archived_and_only_the_newest_are_kept() {
        let dir = Dir::new("archive");
        let summary = dir.0.join("summary.json");
        std::fs::write(&summary, "{}").unwrap();
        let history = dir.0.join("history");
        for t in 1..=4 {
            archive_run(&summary, &history, &run_name(t, Some("abcdef0")), 3).unwrap();
        }
        std::fs::create_dir_all(history.join("notes")).unwrap();
        let mut left: Vec<String> = std::fs::read_dir(&history)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "0000000002-abcdef0",
                "0000000003-abcdef0",
                "0000000004-abcdef0",
                "notes"
            ]
        );
        assert!(history.join("0000000004-abcdef0").join(SUMMARY).is_file());
        assert!(archive_run(&summary, &history, "../x", 3).is_err());
    }
}
