//! Сверка с эталонными файлами в tests/golden. Обновить эталоны после намеренной
//! правки вида: `NIXYON_BLESS=1 cargo test -p nixyon-render`, затем просмотреть diff.

use std::path::PathBuf;

use nixyon_core::{Transcript, from_json};
use std::io::Read;

use nixyon_render::{
    Labels, MAX_CUE_SEC, MAX_LINE_CHARS, MAX_LINES, document_xml, protocol_blocks, render_docx,
    render_json, render_markdown, render_srt, render_txt,
};

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

fn sample() -> Transcript {
    from_json(&std::fs::read_to_string(golden("sample.json")).unwrap()).unwrap()
}

fn ru() -> Labels {
    Labels {
        title: "Расшифровка".into(),
        recorded: "Запись".into(),
        duration: "Длительность".into(),
        participants: "Участники".into(),
        participant: "Участник".into(),
        talk_time: "Время речи".into(),
        share: "Доля".into(),
        percent: "{value}\u{a0}%".into(),
        speaker_default: "Спикер {n}".into(),
    }
}

fn en() -> Labels {
    Labels {
        title: "Transcript".into(),
        recorded: "Recorded".into(),
        duration: "Duration".into(),
        participants: "Participants".into(),
        participant: "Participant".into(),
        talk_time: "Talk time".into(),
        share: "Share".into(),
        percent: "{value}%".into(),
        speaker_default: "Speaker {n}".into(),
    }
}

fn check(name: &str, actual: &str) {
    let path = golden(name);
    if std::env::var_os("NIXYON_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("нет эталона {name}: запустите с NIXYON_BLESS=1"));
    assert_eq!(actual, expected, "{name} не совпал с эталоном");
}

#[test]
fn txt_matches_golden_in_both_languages() {
    let t = sample();
    check(
        "ru.txt",
        &render_txt(&t, &ru(), Some("28 окт. 2026 г., 14:30")),
    );
    check("en.txt", &render_txt(&t, &en(), None));
}

#[test]
fn markdown_matches_golden_in_both_languages() {
    let t = sample();
    check(
        "ru.md",
        &render_markdown(&t, &ru(), Some("28 окт. 2026 г., 14:30")),
    );
    check(
        "en.md",
        &render_markdown(&t, &en(), Some("Oct 28, 2026, 2:30 PM")),
    );
}

#[test]
fn srt_matches_golden_in_both_languages() {
    let t = sample();
    check("ru.srt", &render_srt(&t, &ru()));
    check("en.srt", &render_srt(&t, &en()));
}

#[test]
fn protocol_blocks_match_golden_in_both_languages() {
    let t = sample();
    let ru = protocol_blocks(&t, &ru(), Some("28 окт. 2026 г., 14:30"));
    let en = protocol_blocks(&t, &en(), None);
    check(
        "ru.protocol.json",
        &serde_json::to_string_pretty(&ru).unwrap(),
    );
    check(
        "en.protocol.json",
        &serde_json::to_string_pretty(&en).unwrap(),
    );
    check("ru.docx.xml", &document_xml(&ru));
    check("en.docx.xml", &document_xml(&en));
}

#[test]
fn docx_holds_the_three_package_parts_and_the_same_bytes_every_time() {
    let blocks = protocol_blocks(&sample(), &ru(), Some("28 окт. 2026 г., 14:30"));
    let bytes = render_docx(&blocks).unwrap();
    assert_eq!(bytes, render_docx(&blocks).unwrap());
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["[Content_Types].xml", "_rels/.rels", "word/document.xml"]
    );
    let mut document = String::new();
    archive
        .by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut document)
        .unwrap();
    assert_eq!(document, document_xml(&blocks));
}

#[test]
fn json_export_holds_only_the_transcript_lines() {
    let t = sample();
    let value: serde_json::Value = serde_json::from_str(&render_json(&t).unwrap()).unwrap();
    let root = value.as_object().unwrap();
    assert_eq!(root.len(), 1, "в корне только transcript: {root:?}");
    let lines = root["transcript"].as_array().unwrap();
    assert_eq!(lines.len(), t.turns.len());
    for (line, turn) in lines.iter().zip(&t.turns) {
        let line = line.as_object().unwrap();
        assert_eq!(line.len(), 4, "в реплике только четыре поля: {line:?}");
        for key in ["speaker_id", "start_ms", "end_ms", "text"] {
            assert!(line.contains_key(key), "нет поля {key}");
        }
        assert_eq!(
            line["speaker_id"].as_str(),
            Some(format!("speaker_{}", turn.speaker).as_str())
        );
        assert_eq!(
            line["start_ms"].as_i64(),
            Some((turn.start * 1000.0).round() as i64)
        );
        assert_eq!(
            line["end_ms"].as_i64(),
            Some((turn.end * 1000.0).round() as i64)
        );
        assert_eq!(line["text"].as_str(), Some(turn.text.as_str()));
    }
}

struct Cue {
    start: f64,
    end: f64,
    lines: Vec<String>,
}

fn parse_srt(text: &str) -> Vec<Cue> {
    text.split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .enumerate()
        .map(|(i, block)| {
            let mut lines = block.lines();
            assert_eq!(lines.next().unwrap(), (i + 1).to_string());
            let (a, b) = lines.next().unwrap().split_once(" --> ").unwrap();
            let seconds = |s: &str| {
                let (hms, ms) = s.split_once(',').unwrap();
                let p: Vec<f64> = hms.split(':').map(|x| x.parse().unwrap()).collect();
                p[0] * 3600.0 + p[1] * 60.0 + p[2] + ms.parse::<f64>().unwrap() / 1000.0
            };
            Cue {
                start: seconds(a),
                end: seconds(b),
                lines: lines.map(str::to_string).collect(),
            }
        })
        .collect()
}

#[test]
fn every_srt_cue_keeps_the_limits_and_no_word_is_lost() {
    let t = sample();
    let cues = parse_srt(&render_srt(&t, &ru()));
    for cue in &cues {
        assert!(
            cue.lines.len() <= MAX_LINES,
            "строк больше двух: {:?}",
            cue.lines
        );
        for line in &cue.lines {
            let one_word = !line.contains(' ');
            assert!(
                line.chars().count() <= MAX_LINE_CHARS || one_word,
                "длинная строка: {line}"
            );
        }
        assert!(cue.end > cue.start);
        assert!(
            cue.end - cue.start <= MAX_CUE_SEC + 0.001,
            "титр длиннее 6 с"
        );
    }
    let shown: Vec<String> = cues
        .iter()
        .flat_map(|c| c.lines.iter().flat_map(|l| l.split(' ')))
        .map(str::to_string)
        .collect();
    let spoken: Vec<String> = t
        .turns
        .iter()
        .flat_map(|turn| turn.text.split_whitespace().map(str::to_string))
        .collect();
    let names = ["Спикер", "1:", "Анна", "|", "*главная*:"];
    let shown: Vec<String> = shown
        .into_iter()
        .filter(|w| !names.contains(&w.as_str()))
        .collect();
    assert_eq!(shown, spoken);
}
