use std::path::Path;

use nixyon_core::VoiceSpan;
use serde::Deserialize;

use crate::SegmentsError;

/// Отрезок нейросегментации в том виде, в каком его пишет Kotlin (SpansWriter).
#[derive(Deserialize)]
struct SpanDto {
    start: f64,
    end: f64,
    embedding: Vec<f32>,
}

/// Читает отрезки нейросегментации. Отрезок с концом раньше начала или с нечисловым
/// временем значит битые данные.
pub fn read_spans(path: &Path) -> Result<Vec<VoiceSpan>, SegmentsError> {
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    let dtos: Vec<SpanDto> = serde_json::from_reader(file)?;
    dtos.into_iter()
        .enumerate()
        .map(|(index, d)| {
            if !d.start.is_finite() || !d.end.is_finite() || d.end < d.start {
                return Err(SegmentsError::BadSpan { index });
            }
            Ok(VoiceSpan {
                start: d.start,
                end: d.end,
                embedding: d.embedding,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, body: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("nixyon-spans-{name}-{}", std::process::id()));
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn spans_written_by_kotlin_are_read_back() {
        let path = write(
            "ok",
            r#"[{"start":1.0,"end":2.5,"embedding":[0.5,-0.25]},{"start":2.0,"end":3.0,"embedding":[1.0]}]"#,
        );
        let spans = read_spans(&path).unwrap();
        assert_eq!(
            spans[0],
            VoiceSpan {
                start: 1.0,
                end: 2.5,
                embedding: vec![0.5, -0.25]
            }
        );
        assert_eq!(spans.len(), 2);
        assert!(read_spans(&write("empty", "[]")).unwrap().is_empty());
    }

    #[test]
    fn broken_spans_are_refused() {
        assert!(matches!(
            read_spans(&write(
                "back",
                r#"[{"start":2.0,"end":1.0,"embedding":[]}]"#
            )),
            Err(SegmentsError::BadSpan { index: 0 })
        ));
        assert!(matches!(
            read_spans(&write("cut", "[{\"start\":")),
            Err(SegmentsError::Json(_))
        ));
        assert!(matches!(
            read_spans(Path::new("/nonexistent/nixyon-spans.json")),
            Err(SegmentsError::Io(_))
        ));
    }
}
