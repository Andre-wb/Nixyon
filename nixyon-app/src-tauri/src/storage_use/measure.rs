//! Раскладка места по частям: какие папки и файлы к какой части относятся.

use std::path::Path;

use super::disk_size::on_disk;
use super::usage::StorageUsage;
use crate::meetings::{AUDIO, INDEX_FILE, MEETINGS_DIR};
use crate::vault::{RECORDINGS_DIR, temporary_files};

const VOICES_FILE: &str = "voices.json";
/// Папки скачанных моделей на общей памяти (`ModelTarget.kt`).
const MODEL_DIRS: [&str; 2] = ["models", "llm"];

pub fn measure(files_dir: &Path, cache: &Path, external_files: Option<&Path>) -> StorageUsage {
    let meetings = files_dir.join(MEETINGS_DIR);
    let audio = meetings_audio(&meetings);
    StorageUsage {
        recordings: audio.saturating_add(on_disk(&files_dir.join(RECORDINGS_DIR))),
        transcripts: on_disk(&meetings)
            .saturating_sub(audio)
            .saturating_add(on_disk(&files_dir.join(INDEX_FILE))),
        models: external_files.map_or(0, |dir| {
            MODEL_DIRS
                .iter()
                .map(|name| on_disk(&dir.join(name)))
                .fold(0, u64::saturating_add)
        }),
        voices: on_disk(&files_dir.join(VOICES_FILE)),
        temporary: temporary_files::list(files_dir, cache)
            .iter()
            .map(|path| on_disk(path))
            .fold(0, u64::saturating_add),
    }
}

fn meetings_audio(meetings: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(meetings) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| on_disk(&entry.path().join(AUDIO)))
        .fold(0, u64::saturating_add)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &Path, len: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7u8; len]).unwrap();
    }

    #[test]
    fn each_file_goes_to_its_own_part() {
        let store = crate::vault::test_store("storage-measure");
        let files = store.dir().join("files");
        let cache = store.dir().join("cache");
        let external = store.dir().join("external");
        file(&files.join("meetings/20261107T100000Z/audio.wav"), 300_000);
        file(
            &files.join("meetings/20261107T100000Z/transcript.json"),
            20_000,
        );
        file(&files.join("recordings/rec-1.wav"), 100_000);
        file(&files.join("meetings.json"), 1_000);
        file(&files.join("voices.json"), 2_000);
        file(&files.join("settings.json"), 9_000_000);
        file(&external.join("models/asr.onnx"), 700_000);
        file(&external.join("llm/model.gguf"), 900_000);
        file(&external.join("встреча.m4a"), 5_000_000);
        file(&cache.join("nixyon-1.wav"), 40_000);
        file(&cache.join("WebView/data"), 6_000_000);

        let usage = measure(&files, &cache, Some(&external));
        let audio = on_disk(&files.join("meetings/20261107T100000Z/audio.wav"));
        assert_eq!(usage.recordings, audio + on_disk(&files.join("recordings")));
        assert!((300_000..400_000).contains(&audio));
        assert!(
            (20_000..40_000).contains(&(usage.transcripts - on_disk(&files.join("meetings.json"))))
        );
        assert!(
            (1_600_000..1_700_000).contains(&usage.models),
            "{}",
            usage.models
        );
        assert_eq!(usage.voices, on_disk(&files.join("voices.json")));
        assert_eq!(usage.temporary, on_disk(&cache.join("nixyon-1.wav")));
    }

    #[test]
    fn empty_storage_and_no_shared_folder_take_nothing() {
        let store = crate::vault::test_store("storage-measure-empty");
        let usage = measure(&store.dir().join("files"), &store.dir().join("cache"), None);
        assert_eq!(usage, StorageUsage::default());
    }

    #[test]
    fn removing_audio_moves_nothing_into_transcripts() {
        let store = crate::vault::test_store("storage-measure-no-audio");
        let files = store.dir().join("files");
        let cache = store.dir().join("cache");
        file(&files.join("meetings/20261107T100000Z/audio.wav"), 300_000);
        file(
            &files.join("meetings/20261107T100000Z/transcript.json"),
            20_000,
        );
        let before = measure(&files, &cache, None);
        std::fs::remove_file(files.join("meetings/20261107T100000Z/audio.wav")).unwrap();
        let after = measure(&files, &cache, None);
        assert_eq!(after.recordings, 0);
        assert_eq!(after.transcripts, before.transcripts);
    }
}
