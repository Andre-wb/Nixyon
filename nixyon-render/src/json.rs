use nixyon_core::{CoreError, Transcript};
use serde::Serialize;

/// Реплика экспорта: только спикер, время в миллисекундах и текст.
#[derive(Serialize)]
struct Line<'a> {
    speaker_id: String,
    start_ms: i64,
    end_ms: i64,
    text: &'a str,
}

/// Корень файла: единственный ключ `transcript`, служебных полей расшифровки нет.
#[derive(Serialize)]
struct Export<'a> {
    transcript: Vec<Line<'a>>,
}

/// JSON экспорта: реплики по схеме `{"transcript": [...]}` без слов, флагов и метаданных.
pub fn render_json(transcript: &Transcript) -> Result<String, CoreError> {
    let export = Export {
        transcript: transcript
            .turns
            .iter()
            .map(|turn| Line {
                speaker_id: format!("speaker_{}", turn.speaker),
                start_ms: milliseconds(turn.start),
                end_ms: milliseconds(turn.end),
                text: &turn.text,
            })
            .collect(),
    };
    Ok(serde_json::to_string_pretty(&export)?)
}

fn milliseconds(seconds: f64) -> i64 {
    (seconds * 1000.0).round() as i64
}
