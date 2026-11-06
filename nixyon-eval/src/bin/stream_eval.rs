//! Замер потоковых моделей для живого текста. Звук записи подаётся кусками по 0,1 с, как
//! с микрофона, без перекодирования; текст фраз сравнивается с эталоном (WER, CER), время
//! обработки делится на длину звука.
//!
//! `cargo run --release -p nixyon-eval --bin stream_eval -- [--threads N] <папка записей>
//! <имя,имя> <вид=папка> [<вид=папка>...]`, вид: `zipformer` или `t-one`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use nixyon_eval::Truth;
use nixyon_metrics::{ErrorCount, char_errors, word_errors};
use nixyon_sherpa::{LiveText, StreamModel};
use sherpa_onnx::Wave;

const CHUNK_SEC: f32 = 0.1;

struct Args {
    threads: i32,
    records: PathBuf,
    names: Vec<String>,
    models: Vec<StreamModel>,
}

struct Row {
    wer: ErrorCount,
    cer: ErrorCount,
    audio_sec: f64,
    processing_sec: f64,
    phrases: usize,
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1).collect()) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    match run(&args) {
        Ok(table) => {
            print!("{table}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(raw: Vec<String>) -> Result<Args, String> {
    let mut threads = 1;
    let mut positional = Vec::new();
    let mut iter = raw.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "--threads" {
            threads = iter
                .next()
                .and_then(|v| v.parse().ok())
                .filter(|&n: &i32| n >= 1)
                .ok_or("--threads: целое от 1")?;
        } else {
            positional.push(arg);
        }
    }
    let mut positional = positional.into_iter();
    let (Some(records), Some(names)) = (positional.next(), positional.next()) else {
        return Err("нужны папка записей, имена записей и хотя бы одна модель".into());
    };
    let models = positional
        .map(|spec| {
            StreamModel::parse(&spec).ok_or(format!(
                "модель {spec}: zipformer=<папка> или t-one=<папка>"
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if models.is_empty() {
        return Err("нужна хотя бы одна модель".into());
    }
    Ok(Args {
        threads,
        records: records.into(),
        names: names.split(',').map(str::to_owned).collect(),
        models,
    })
}

fn run(args: &Args) -> Result<String, Box<dyn std::error::Error>> {
    let missing: Vec<String> = args
        .names
        .iter()
        .flat_map(|n| [format!("{n}.wav"), format!("{n}.truth.json")])
        .map(|f| args.records.join(f))
        .chain(
            args.models
                .iter()
                .flat_map(|m| m.files().into_iter().map(Path::to_path_buf)),
        )
        .filter(|p| !p.is_file())
        .map(|p| p.display().to_string())
        .collect();
    if !missing.is_empty() {
        return Err(format!("нет файлов: {}", missing.join(", ")).into());
    }
    let mut out = String::from(
        "| модель | запись | WER | CER | фраз | звук, с | обработка, с | RTF |\n|---|---|---|---|---|---|---|---|\n",
    );
    for model in &args.models {
        let mut rows = Vec::new();
        for name in &args.names {
            let row = measure(model, args, name)?;
            out.push_str(&line(model.name(), name, &row));
            rows.push(row);
        }
        let total = Row {
            wer: rows.iter().map(|r| r.wer).sum(),
            cer: rows.iter().map(|r| r.cer).sum(),
            audio_sec: rows.iter().map(|r| r.audio_sec).sum(),
            processing_sec: rows.iter().map(|r| r.processing_sec).sum(),
            phrases: rows.iter().map(|r| r.phrases).sum(),
        };
        out.push_str(&line(model.name(), "всего", &total));
    }
    Ok(out)
}

fn measure(
    model: &StreamModel,
    args: &Args,
    name: &str,
) -> Result<Row, Box<dyn std::error::Error>> {
    let truth_path = args.records.join(format!("{name}.truth.json"));
    let truth = Truth::from_json(&std::fs::read_to_string(&truth_path)?)?;
    let reference = truth
        .text()
        .ok_or_else(|| format!("{}: у реплик нет текста", truth_path.display()))?;
    let wave = read(&args.records.join(format!("{name}.wav")))?;
    let rate = wave.sample_rate();
    let samples = wave.samples();
    // Частота звука мала и положительна, приведение без потерь по смыслу.
    let chunk = ((rate as f32 * CHUNK_SEC).round() as usize).max(1);

    let started = Instant::now();
    let mut live = LiveText::new(model, args.threads, rate)?;
    for piece in samples.chunks(chunk) {
        live.accept(rate, piece);
    }
    let phrases = live.finish(rate);
    let processing_sec = started.elapsed().as_secs_f64();

    let hypothesis = phrases.join(" ");
    Ok(Row {
        wer: word_errors(&reference, &hypothesis),
        cer: char_errors(&reference, &hypothesis),
        // Длина записи в отсчётах далеко от 2^53, перевод точный.
        audio_sec: samples.len() as f64 / f64::from(rate),
        processing_sec,
        phrases: phrases.len(),
    })
}

fn read(path: &Path) -> Result<Wave, String> {
    Wave::read(&path.to_string_lossy())
        .ok_or_else(|| format!("{}: WAV не читается", path.display()))
}

fn line(model: &str, record: &str, row: &Row) -> String {
    let percent = |c: ErrorCount| {
        c.rate()
            .map_or("нет".into(), |r| format!("{:.1} %", r * 100.0))
    };
    format!(
        "| {model} | {record} | {} | {} | {} | {:.1} | {:.1} | {:.3} |\n",
        percent(row.wer),
        percent(row.cer),
        row.phrases,
        row.audio_sec,
        row.processing_sec,
        row.processing_sec / row.audio_sec,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &[&str]) -> Result<Args, String> {
        parse_args(raw.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn records_names_and_models_are_parsed() {
        let args = parse(&["--threads", "2", "dir", "ru2,ru4", "zipformer=z", "t-one=t"]).unwrap();
        assert_eq!(args.threads, 2);
        assert_eq!(args.names, ["ru2", "ru4"]);
        assert_eq!(
            args.models,
            [
                StreamModel::zipformer_dir(Path::new("z")),
                StreamModel::t_one_dir(Path::new("t"))
            ]
        );
    }

    #[test]
    fn unknown_model_or_missing_model_is_an_error() {
        assert!(parse(&["dir", "ru2", "vosk=v"]).is_err());
        assert!(parse(&["dir", "ru2"]).is_err());
        assert!(parse(&["--threads", "0", "dir", "ru2", "t-one=t"]).is_err());
    }
}
