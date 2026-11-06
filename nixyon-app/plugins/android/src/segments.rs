use std::path::Path;

use nixyon_core::{SegmentResult, WindowEmbedding};
use serde::Deserialize;

/// Кусок речи в том виде, в каком его пишет Kotlin (SegmentsWriter).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SegmentDto {
    start: f64,
    end: f64,
    text: String,
    tokens: Vec<String>,
    token_times: Vec<f32>,
    embedding: Vec<f32>,
    /// Старые файлы и разбор без окон: поля нет или массив пуст.
    #[serde(default)]
    windows: Vec<WindowDto>,
}

#[derive(Deserialize)]
struct WindowDto {
    start: f64,
    end: f64,
    embedding: Vec<f32>,
}

#[derive(Debug, thiserror::Error)]
pub enum SegmentsError {
    #[error("файл с результатами разбора не читается: {0}")]
    Io(#[from] std::io::Error),
    #[error("результаты разбора не разбираются: {0}")]
    Json(#[from] serde_json::Error),
    #[error("у куска речи {index} токенов {tokens}, а времён {times}")]
    Mismatch {
        index: usize,
        tokens: usize,
        times: usize,
    },
    #[error("у отрезка нейросегментации {index} неверное время")]
    BadSpan { index: usize },
}

pub fn read_segments(path: &Path) -> Result<Vec<SegmentResult>, SegmentsError> {
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    let dtos: Vec<SegmentDto> = serde_json::from_reader(file)?;
    dtos.into_iter()
        .enumerate()
        .map(|(index, d)| {
            // Время каждого токена нужно для слов: разная длина значит битые данные.
            if d.tokens.len() != d.token_times.len() {
                return Err(SegmentsError::Mismatch {
                    index,
                    tokens: d.tokens.len(),
                    times: d.token_times.len(),
                });
            }
            Ok(SegmentResult {
                start: d.start,
                end: d.end,
                text: d.text,
                tokens: d.tokens,
                token_times: d.token_times,
                embedding: d.embedding,
                windows: d
                    .windows
                    .into_iter()
                    .map(|w| WindowEmbedding {
                        start: w.start,
                        end: w.end,
                        embedding: w.embedding,
                    })
                    .collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, body: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("nixyon-segments-{name}-{}", std::process::id()));
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn segments_written_by_kotlin_are_read_back() {
        let path = write(
            "ok",
            r#"[{"start":1.5,"end":2.25,"text":"да","tokens":["д","а"],"tokenTimes":[0.0,0.04],"embedding":[0.5,-0.25]},
                {"start":3.0,"end":3.1,"text":"","tokens":[],"tokenTimes":[],"embedding":[]}]"#,
        );
        let segments = read_segments(&path).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].tokens, ["д", "а"]);
        assert_eq!(segments[0].token_times, [0.0, 0.04]);
        assert_eq!(segments[0].embedding, [0.5, -0.25]);
        assert!(segments[1].embedding.is_empty());
        assert!(segments[0].windows.is_empty());
    }

    #[test]
    fn window_embeddings_are_read_with_their_times() {
        let path = write(
            "windows",
            r#"[{"start":1.0,"end":3.0,"text":"да","tokens":[],"tokenTimes":[],"embedding":[1.0],
                 "windows":[{"start":1.0,"end":2.5,"embedding":[0.5,0.5]},{"start":1.5,"end":3.0,"embedding":[0.25,0.75]}]}]"#,
        );
        let windows = &read_segments(&path).unwrap()[0].windows;
        assert_eq!(windows.len(), 2);
        assert_eq!(
            windows[1],
            WindowEmbedding {
                start: 1.5,
                end: 3.0,
                embedding: vec![0.25, 0.75]
            }
        );
    }

    #[test]
    fn broken_or_inconsistent_results_are_refused() {
        let mismatch = write(
            "mismatch",
            r#"[{"start":0,"end":1,"text":"а","tokens":["а"],"tokenTimes":[],"embedding":[]}]"#,
        );
        assert!(matches!(
            read_segments(&mismatch),
            Err(SegmentsError::Mismatch {
                index: 0,
                tokens: 1,
                times: 0
            })
        ));
        assert!(matches!(
            read_segments(&write("cut", "[{\"start\":")),
            Err(SegmentsError::Json(_))
        ));
        assert!(matches!(
            read_segments(Path::new("/нет/такого/файла")),
            Err(SegmentsError::Io(_))
        ));
    }
}
