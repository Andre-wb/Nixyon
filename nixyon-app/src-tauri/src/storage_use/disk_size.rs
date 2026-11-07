//! Сколько места файл или папка занимают на диске на самом деле: по занятым блокам, а не
//! по длине файлов. Ссылки не раскрываются: считается сама ссылка, не то, на что она ведёт.

use std::fs::Metadata;
use std::path::Path;

/// Нет пути: ноль. Папка, которая не читается, считается без содержимого, причина в логе.
pub fn on_disk(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    let own = allocated(&meta);
    if !meta.is_dir() {
        return own;
    }
    match std::fs::read_dir(path) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| on_disk(&entry.path()))
            .fold(own, u64::saturating_add),
        Err(e) => {
            log::warn!("папка не читается, размер без неё: {e}");
            own
        }
    }
}

#[cfg(unix)]
fn allocated(meta: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    // `blocks` всегда в единицах по 512 байт, какой бы ни был блок файловой системы.
    meta.blocks().saturating_mul(512)
}

#[cfg(not(unix))]
fn allocated(meta: &Metadata) -> u64 {
    meta.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_counts_whole_blocks_of_every_file_in_nested_folders() {
        let store = crate::vault::test_store("disk-size");
        let dir = store.dir().join("a");
        std::fs::create_dir_all(dir.join("b")).unwrap();
        std::fs::write(dir.join("one.bin"), vec![1u8; 10]).unwrap();
        std::fs::write(dir.join("b/two.bin"), vec![2u8; 5_000]).unwrap();
        let one = on_disk(&dir.join("one.bin"));
        let two = on_disk(&dir.join("b/two.bin"));
        // Файл в 10 байт занимает целый блок, а не 10 байт.
        assert!(one >= 512 && one.is_multiple_of(512), "{one}");
        assert!(two >= 5_000 && two.is_multiple_of(512), "{two}");
        assert!(on_disk(&dir) >= one + two);
    }

    #[test]
    fn missing_path_takes_no_space() {
        let store = crate::vault::test_store("disk-size-missing");
        assert_eq!(on_disk(&store.dir().join("нет")), 0);
    }

    #[cfg(unix)]
    #[test]
    fn link_is_not_followed() {
        let store = crate::vault::test_store("disk-size-link");
        let big = store.dir().join("big.bin");
        std::fs::write(&big, vec![0u8; 200_000]).unwrap();
        let link = store.dir().join("link");
        std::os::unix::fs::symlink(&big, &link).unwrap();
        assert!(on_disk(&link) < on_disk(&big));
    }
}
