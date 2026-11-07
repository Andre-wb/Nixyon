use std::path::{Path, PathBuf};

use super::meeting_id::MeetingId;

/// Папка архива в `files`: в ней по папке на встречу.
pub const MEETINGS_DIR: &str = "meetings";
pub const TRANSCRIPT: &str = "transcript.json";
pub const CENTERS: &str = "centers.json";
pub const SUMMARY: &str = "summary.json";
pub const AUDIO: &str = "audio.wav";

/// Папка одной встречи, `meetings/<id без разделителей>`.
pub struct MeetingDir {
    relative: String,
}

impl MeetingDir {
    pub fn of(id: &MeetingId) -> Self {
        Self {
            relative: format!("{MEETINGS_DIR}/{}", id.dir_name()),
        }
    }

    /// Имя файла встречи для `FilesStore`, относительно `files`.
    pub fn file(&self, name: &str) -> String {
        format!("{}/{name}", self.relative)
    }

    pub fn path(&self, files_dir: &Path) -> PathBuf {
        files_dir.join(&self.relative)
    }
}
