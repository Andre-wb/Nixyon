//! Экспорт расшифровки встречи: файл нужного формата строится в `cache/exports`, а
//! сохранить его в выбранное место или отправить в другое приложение помогает Kotlin.
//! PDF верстает Kotlin по блокам протокола, остальные форматы собирает Rust.

use std::path::{Path, PathBuf};

use nixyon_android::{SpeechExt, StoragePaths};
use nixyon_core::Transcript;
use nixyon_locales::{Lang, export_labels};
use nixyon_render::{
    protocol_blocks, render_docx, render_json, render_markdown, render_srt, render_txt,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::transcription::CommandError;

const EXPORT_FAILED: &str = "export_failed";
const MAX_STEM_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Txt,
    Md,
    Srt,
    Json,
    Docx,
    Pdf,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Format::Txt => "txt",
            Format::Md => "md",
            Format::Srt => "srt",
            Format::Json => "json",
            Format::Docx => "docx",
            Format::Pdf => "pdf",
        }
    }

    fn mime(self) -> &'static str {
        match self {
            Format::Txt => "text/plain",
            Format::Md => "text/markdown",
            Format::Srt => "application/x-subrip",
            Format::Json => "application/json",
            Format::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            Format::Pdf => "application/pdf",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ExportedFile {
    pub path: String,
    pub name: String,
    pub mime: &'static str,
}

/// Имя файла экспорта: имя исходной записи без расширения и формат. Без знаков пути и
/// запрещённых в именах файлов символов; пустое имя даёт «transcript».
pub fn file_name(source: &str, format: Format) -> String {
    let last = source.rsplit(['/', '\\']).next().unwrap_or_default();
    let stem = last.rsplit_once('.').map_or(last, |(stem, _)| stem);
    let clean: String = stem
        .chars()
        .filter(|c| !c.is_control() && !r#"<>:"/\|?*"#.contains(*c))
        .take(MAX_STEM_CHARS)
        .collect();
    let clean = clean.trim().trim_start_matches('.');
    let stem = if clean.is_empty() {
        "transcript"
    } else {
        clean
    };
    format!("{stem}.{}", format.extension())
}

/// Готовое содержимое файла или блоки протокола в JSON, которые Kotlin сверстает в PDF.
#[derive(Debug, PartialEq, Eq)]
pub enum Rendered {
    File(Vec<u8>),
    PdfBlocks(String),
}

fn failed<E: std::fmt::Display>(e: E) -> CommandError {
    CommandError::logged(EXPORT_FAILED, e)
}

pub fn render(
    transcript: &Transcript,
    format: Format,
    lang: Lang,
    recorded_at: Option<&str>,
) -> Result<Rendered, CommandError> {
    let labels = export_labels(lang).map_err(failed)?;
    let text = |body: String| Rendered::File(body.into_bytes());
    Ok(match format {
        Format::Txt => text(render_txt(transcript, &labels, recorded_at)),
        Format::Md => text(render_markdown(transcript, &labels, recorded_at)),
        Format::Srt => text(render_srt(transcript, &labels)),
        Format::Json => text(render_json(transcript).map_err(failed)?),
        Format::Docx => Rendered::File(
            render_docx(&protocol_blocks(transcript, &labels, recorded_at)).map_err(failed)?,
        ),
        Format::Pdf => Rendered::PdfBlocks(
            serde_json::to_string(&protocol_blocks(transcript, &labels, recorded_at))
                .map_err(failed)?,
        ),
    })
}

fn exports_dir(dirs: &StoragePaths) -> PathBuf {
    Path::new(&dirs.cache).join("exports")
}

/// Новый экспорт убирает прежние файлы: в папке всегда только последний.
fn fresh_export_path(dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    if dir.exists() {
        for entry in std::fs::read_dir(dir)?.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    std::fs::create_dir_all(dir)?;
    Ok(dir.join(name))
}

fn write_export(dir: &Path, name: &str, body: &[u8]) -> std::io::Result<PathBuf> {
    let path = fresh_export_path(dir, name)?;
    std::fs::write(&path, body)?;
    Ok(path)
}

/// Отдать в Kotlin можно только файл из папки экспорта: путь приходит из интерфейса.
pub fn checked_export(path: &str, dir: &Path) -> Option<PathBuf> {
    let resolved = Path::new(path).canonicalize().ok()?;
    let dir = dir.canonicalize().ok()?;
    (resolved.is_file() && resolved.parent() == Some(dir.as_path())).then_some(resolved)
}

async fn dirs<R: Runtime>(app: &AppHandle<R>) -> Result<StoragePaths, CommandError> {
    app.speech()
        .storage_paths()
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))
}

/// Строит файл экспорта расшифровки встречи на языке интерфейса. Дату записи
/// присылает интерфейс уже в поясе устройства.
#[tauri::command]
pub async fn export_transcript<R: Runtime>(
    app: AppHandle<R>,
    meeting: String,
    format: Format,
    language: String,
    recorded_at: Option<String>,
) -> Result<ExportedFile, CommandError> {
    let storage = crate::vault::storage(&app).await?;
    let store = storage.store;
    let lang = Lang::from_code(&language).unwrap_or(Lang::Ru);
    let dir = exports_dir(&storage.paths);
    let (name, path, pdf_blocks) = tauri::async_runtime::spawn_blocking(move || {
        let (_, transcript) = crate::meetings::load(&store, &meeting)?;
        let rendered = render(&transcript, format, lang, recorded_at.as_deref())?;
        let name = file_name(&transcript.audio.file, format);
        match rendered {
            Rendered::File(body) => {
                let path = write_export(&dir, &name, &body).map_err(failed)?;
                Ok::<_, CommandError>((name, path, None))
            }
            Rendered::PdfBlocks(blocks) => {
                let path = fresh_export_path(&dir, &name).map_err(failed)?;
                Ok((name, path, Some(blocks)))
            }
        }
    })
    .await
    .map_err(failed)??;
    let path = path.to_string_lossy().into_owned();
    if let Some(blocks) = pdf_blocks {
        app.speech()
            .render_pdf(path.clone(), blocks)
            .await
            .map_err(plugin_error(EXPORT_FAILED))?;
    }
    Ok(ExportedFile {
        path,
        name,
        mime: format.mime(),
    })
}

fn plugin_error(code: &'static str) -> impl Fn(nixyon_android::Error) -> CommandError {
    move |e| CommandError::logged(code, e)
}

/// Системный диалог «Сохранить как»: true, если файл сохранён, false, если отменено.
#[tauri::command]
pub async fn save_export<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    name: String,
    mime: String,
) -> Result<bool, CommandError> {
    let dirs = dirs(&app).await?;
    let file = checked_export(&path, &exports_dir(&dirs))
        .ok_or_else(|| CommandError::new(CommandError::BAD_PATH))?;
    app.speech()
        .save_file(file.to_string_lossy().into_owned(), name, mime)
        .await
        .map_err(plugin_error("save_failed"))
}

/// Системное меню «Поделиться» с файлом экспорта.
#[tauri::command]
pub async fn share_export<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    name: String,
    mime: String,
) -> Result<(), CommandError> {
    let dirs = dirs(&app).await?;
    let file = checked_export(&path, &exports_dir(&dirs))
        .ok_or_else(|| CommandError::new(CommandError::BAD_PATH))?;
    app.speech()
        .share_file(file.to_string_lossy().into_owned(), name, mime)
        .await
        .map_err(plugin_error("share_failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_name_keeps_the_source_name_and_drops_unsafe_parts() {
        assert_eq!(file_name("ru2.m4a", Format::Srt), "ru2.srt");
        assert_eq!(
            file_name("Запись 2026-10-29 14-30", Format::Txt),
            "Запись 2026-10-29 14-30.txt"
        );
        assert_eq!(file_name("../../etc/passwd", Format::Json), "passwd.json");
        assert_eq!(file_name("a:b*c?.mp3", Format::Md), "abc.md");
        assert_eq!(file_name(".hidden", Format::Txt), "transcript.txt");
        assert_eq!(file_name("ru2.m4a", Format::Docx), "ru2.docx");
        assert_eq!(file_name("ru2.m4a", Format::Pdf), "ru2.pdf");
        assert_eq!(file_name("", Format::Txt), "transcript.txt");
        assert_eq!(
            file_name(&"я".repeat(300), Format::Txt).chars().count(),
            MAX_STEM_CHARS + 4
        );
    }

    #[test]
    fn only_the_latest_export_stays_and_only_it_can_be_handed_out() {
        let base = std::env::temp_dir().join(format!("nixyon-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("exports");
        let first = write_export(&dir, "a.txt", b"1").unwrap();
        let second = write_export(&dir, "b.srt", b"2").unwrap();
        assert!(!first.exists());
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "2");
        assert!(checked_export(second.to_str().unwrap(), &dir).is_some());
        std::fs::write(base.join("secret.json"), "x").unwrap();
        let outside = format!("{}/../secret.json", dir.display());
        assert!(checked_export(&outside, &dir).is_none());
        assert!(checked_export(dir.to_str().unwrap(), &dir).is_none());
    }

    fn transcript() -> Transcript {
        nixyon_core::from_json(include_str!(
            "../../../nixyon-render/tests/golden/sample.json"
        ))
        .unwrap()
    }

    #[test]
    fn docx_is_a_zip_package_and_pdf_goes_to_kotlin_as_protocol_blocks() {
        let t = transcript();
        let Rendered::File(docx) = render(&t, Format::Docx, Lang::Ru, Some("28 окт.")).unwrap()
        else {
            panic!("DOCX должен собираться в Rust");
        };
        assert!(docx.starts_with(b"PK\x03\x04"));
        let Rendered::PdfBlocks(json) = render(&t, Format::Pdf, Lang::En, Some("Oct 28")).unwrap()
        else {
            panic!("PDF должен уходить в Kotlin блоками");
        };
        let blocks: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(blocks[0]["kind"], "title");
        assert_eq!(blocks[0]["text"], "Transcript");
        assert_eq!(blocks[1]["value"], "Oct 28");
    }

    #[test]
    fn new_formats_come_from_the_interface_by_their_short_names() {
        let docx: Format = serde_json::from_str("\"docx\"").unwrap();
        let pdf: Format = serde_json::from_str("\"pdf\"").unwrap();
        assert_eq!((docx, pdf), (Format::Docx, Format::Pdf));
        assert_eq!(Format::Pdf.mime(), "application/pdf");
    }
}
