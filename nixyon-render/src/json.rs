use nixyon_core::{CoreError, Transcript, Turn};
use serde::Serialize;

/// Файл экспорта JSON в формате задания: массив реплик `transcript`, у каждой
/// `speaker_id` («speaker_N» по номеру спикера), `start_ms` и `end_ms` целыми
/// миллисекундами от начала записи и `text`. Других полей нет. Внутренний формат
/// расшифровки (`nixyon_core::to_json`) остаётся для хранения на устройстве.
#[derive(Serialize)]
struct Export<'a> {
    transcript: Vec<Line<'a>>,
}

#[derive(Serialize)]
struct Line<'a> {
    speaker_id: String,
    start_ms: u64,
    end_ms: u64,
    text: &'a str,
}

/// Реплики идут по времени начала; при равном начале порядок расшифровки сохраняется.
pub fn render_json(transcript: &Transcript) -> Result<String, CoreError> {
    let mut lines: Vec<Line> = transcript.turns.iter().map(line).collect();
    lines.sort_by_key(|l| l.start_ms);
    Ok(serde_json::to_string_pretty(&Export { transcript: lines })?)
}

fn line(turn: &Turn) -> Line<'_> {
    let start_ms = millis(turn.start);
    Line {
        speaker_id: format!("speaker_{}", turn.speaker),
        start_ms,
        end_ms: millis(turn.end).max(start_ms),
        text: turn.text.trim(),
    }
}

/// Секунды в целые миллисекунды с округлением до ближайшего. Отрицательное время и NaN
/// дают 0.
fn millis(sec: f64) -> u64 {
    // `as` насыщает: бесконечность даёт u64::MAX, а не панику.
    (sec.max(0.0) * 1000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION};
    use serde_json::{Value, json};

    fn turn(id: u32, speaker: u32, start: f64, end: f64, text: &str) -> Turn {
        Turn {
            id,
            speaker,
            start,
            end,
            text: text.into(),
            words: None,
            flags: vec![],
        }
    }

    fn transcript(turns: Vec<Turn>) -> Transcript {
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: "2026-11-06T18:00:00Z".into(),
            audio: AudioInfo {
                file: "cats.wav".into(),
                duration_sec: 10.0,
                sample_rate: 16_000,
            },
            engine: EngineInfo {
                vad: "vad.onnx".into(),
                asr: "giga-am-v3".into(),
                embedding: "redimnet2-b6.onnx".into(),
            },
            speakers: vec![],
            turns,
        }
    }

    fn exported(turns: Vec<Turn>) -> Value {
        serde_json::from_str(&render_json(&transcript(turns)).unwrap()).unwrap()
    }

    #[test]
    fn matches_the_example_from_the_task() {
        let value = exported(vec![turn(1, 1, 1.25, 4.38, "Добрый день!")]);
        assert_eq!(
            value,
            json!({
                "transcript": [
                    { "speaker_id": "speaker_1", "start_ms": 1250, "end_ms": 4380, "text": "Добрый день!" }
                ]
            })
        );
    }

    #[test]
    fn times_are_integers_and_only_the_four_fields_are_written() {
        let value = exported(vec![turn(1, 2, 0.0004, 0.0006, "мяу")]);
        let line = value["transcript"][0].as_object().unwrap();
        let mut keys: Vec<&str> = line.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["end_ms", "speaker_id", "start_ms", "text"]);
        assert!(line["start_ms"].is_u64() && line["end_ms"].is_u64());
        assert_eq!(
            (line["start_ms"].as_u64(), line["end_ms"].as_u64()),
            (Some(0), Some(1))
        );
        assert_eq!(value.as_object().unwrap().len(), 1);
    }

    #[test]
    fn lines_go_by_start_time_and_keep_the_speaker_number() {
        let value = exported(vec![
            turn(1, 1, 0.5, 3.0, "первая"),
            turn(2, 3, 5.0, 6.0, "третья"),
            turn(3, 2, 2.0, 4.0, "поверх первой"),
        ]);
        let order: Vec<(&str, &str)> = value["transcript"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                (
                    l["speaker_id"].as_str().unwrap(),
                    l["text"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            order,
            [
                ("speaker_1", "первая"),
                ("speaker_2", "поверх первой"),
                ("speaker_3", "третья"),
            ]
        );
    }

    #[test]
    fn odd_times_never_give_an_end_before_the_start() {
        let value = exported(vec![
            turn(1, 1, -0.3, 1.0, " с пробелами "),
            turn(2, 1, 3.0, 2.0, "конец раньше начала"),
            turn(3, 1, f64::NAN, f64::NAN, "нет времени"),
        ]);
        let times: Vec<(u64, u64)> = value["transcript"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                (
                    l["start_ms"].as_u64().unwrap(),
                    l["end_ms"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(times, [(0, 1000), (0, 0), (3000, 3000)]);
        assert_eq!(value["transcript"][0]["text"], "с пробелами");
    }

    #[test]
    fn an_empty_transcript_is_an_empty_array() {
        assert_eq!(exported(vec![]), json!({ "transcript": [] }));
    }
}
