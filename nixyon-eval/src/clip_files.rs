use std::io::{BufWriter, Write};
use std::path::Path;

use nixyon_metrics::word_alignment;

use crate::{ClipLine, ClipResult, ClipTag, EditCell, clip_tags, clip_to_wav16k};

/// Сколько худших клипов набора сохраняется в WAV, чтобы их можно было прослушать.
pub const WORST_AUDIO: usize = 200;
/// Папки внутри отчёта: строки клипов по наборам и звук худших клипов.
pub const CLIPS_DIR: &str = "clips";
pub const AUDIO_DIR: &str = "audio";

/// Клип набора и путь к его исходному звуку.
#[derive(Debug, Clone, Copy)]
pub struct SetClip<'a> {
    pub result: &'a ClipResult,
    pub audio: &'a Path,
}

/// Сколько WAV записано и сколько не удалось (клип тогда остаётся без звука).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetFiles {
    pub audio_saved: usize,
    pub audio_failed: usize,
}

/// Пишет `clips/<набор>.jsonl` (строка на клип в том порядке, в каком клипы переданы:
/// вызывающий ставит худшие раньше) и `audio/<набор>/<номер>.wav` для первых
/// `WORST_AUDIO` клипов с ошибками. Звук сохраняется только у наборов с текстом: у
/// остальных нет ошибок, по которым выбрать худшие.
pub fn write_set_clips(
    out: &Path,
    set_id: &str,
    has_text: bool,
    clips: &[SetClip],
) -> std::io::Result<SetFiles> {
    let mut files = SetFiles::default();
    let mut audio = vec![None; clips.len()];
    if has_text {
        let dir = out.join(AUDIO_DIR).join(set_id);
        std::fs::create_dir_all(&dir)?;
        let worst = clips
            .iter()
            .enumerate()
            .filter(|(_, c)| c.result.has_real_errors())
            .take(WORST_AUDIO);
        for (n, clip) in worst {
            match clip_to_wav16k(clip.audio, &dir.join(format!("{n}.wav"))) {
                Ok(_) => {
                    audio[n] = Some(n);
                    files.audio_saved += 1;
                }
                Err(_) => files.audio_failed += 1,
            }
        }
    }
    let dir = out.join(CLIPS_DIR);
    std::fs::create_dir_all(&dir)?;
    let mut writer = BufWriter::new(std::fs::File::create(dir.join(format!("{set_id}.jsonl")))?);
    for (clip, audio) in clips.iter().zip(audio) {
        let line = if has_text {
            let edits = word_alignment(&clip.result.reference, &clip.result.hypothesis);
            ClipLine {
                result: clip.result,
                tags: clip_tags(clip.result, &edits),
                edits: Some(edits.into_iter().map(EditCell).collect()),
                audio,
            }
        } else {
            // Без эталона каждое распознанное слово считалось бы лишним.
            let mut tags = clip_tags(clip.result, &[]);
            tags.retain(|t| *t != ClipTag::Errors);
            ClipLine {
                result: clip.result,
                tags,
                edits: None,
                audio: None,
            }
        };
        serde_json::to_writer(&mut writer, &line)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClipHypothesis;
    use std::path::PathBuf;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("nixyon-clip-files-{name}-{}", std::process::id()));
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

    fn heard(id: &str, reference: &str, text: &str) -> ClipResult {
        ClipResult::new(
            id,
            reference,
            1.0,
            0.1,
            Ok(ClipHypothesis {
                text: text.into(),
                speakers: 1,
            }),
        )
    }

    fn silent_wav(path: &Path) {
        let samples = 16_000u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + samples * 2).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&32_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(samples * 2).to_le_bytes());
        bytes.resize(bytes.len() + samples as usize * 2, 0);
        std::fs::write(path, bytes).unwrap();
    }

    fn lines(out: &Path, set: &str) -> Vec<serde_json::Value> {
        std::fs::read_to_string(out.join(CLIPS_DIR).join(format!("{set}.jsonl")))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn clips_with_errors_get_audio_and_every_clip_gets_a_line() {
        let dir = Dir::new("text");
        let source = dir.0.join("a.wav");
        silent_wav(&source);
        let bad = heard("bad", "раз два", "раз");
        let broken = heard("broken", "раз", "два");
        let good = heard("good", "раз", "раз");
        let missing = dir.0.join("missing.wav");
        let clips = [
            SetClip {
                result: &bad,
                audio: &source,
            },
            SetClip {
                result: &broken,
                audio: &missing,
            },
            SetClip {
                result: &good,
                audio: &source,
            },
        ];
        let out = dir.0.join("report");
        let files = write_set_clips(&out, "cv", true, &clips).unwrap();
        assert_eq!(
            files,
            SetFiles {
                audio_saved: 1,
                audio_failed: 1
            }
        );
        assert!(out.join(AUDIO_DIR).join("cv").join("0.wav").is_file());
        let lines = lines(&out, "cv");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["id"], "bad");
        assert_eq!(lines[0]["audio"], 0);
        assert_eq!(lines[0]["tags"], serde_json::json!(["errors", "cut_end"]));
        assert!(lines[1].get("audio").is_none(), "звук не прочитался");
        assert!(lines[2].get("audio").is_none(), "клип без ошибок");
        assert_eq!(lines[2]["edits"], serde_json::json!([["=", "раз"]]));
    }

    #[test]
    fn a_set_without_text_has_no_audio_edits_or_error_tags() {
        let dir = Dir::new("silent");
        let source = dir.0.join("a.wav");
        silent_wav(&source);
        let voice = heard("v", "", "какие-то слова");
        let out = dir.0.join("report");
        let files = write_set_clips(
            &out,
            "voice",
            false,
            &[SetClip {
                result: &voice,
                audio: &source,
            }],
        )
        .unwrap();
        assert_eq!(files, SetFiles::default());
        assert!(!out.join(AUDIO_DIR).exists());
        let lines = lines(&out, "voice");
        assert_eq!(lines[0]["tags"], serde_json::json!([]));
        assert!(lines[0].get("edits").is_none());
    }
}
