//! Замер модели audEERING на устройстве: загрузка из байтов (так модель грузится из APK),
//! пик памяти и время разбора речи спикера.
//! `age_gender_bench <age-gender.onnx> <звук.f32> [секунд речи] [потоков] [повторов] [bytes|file]`.
//! `bytes`: байты файла в память и сессия из них (по умолчанию), `file`: сессия прямо с диска.
//! Звук: сырые 32-битные float, little-endian, 16 кГц, моно (`ffmpeg -f f32le -ar 16000 -ac 1`).
//! По умолчанию 30 с речи, 4 потока (как у модели голоса в приложении), 3 повтора. Речь
//! режется на куски по окну модели, 10 с, как реплики одного спикера.

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use nixyon_ort::AgeGender;

const SAMPLE_RATE: usize = 16_000;
const WINDOW_SEC: f64 = 10.0;
const DEFAULT_SECONDS: usize = 30;
const DEFAULT_THREADS: usize = 4;
const DEFAULT_REPEATS: usize = 3;
const USAGE: &str = "использование: age_gender_bench <age-gender.onnx> <звук.f32> [секунд] \
     [потоков] [повторов] [bytes|file]";

struct Args {
    model: String,
    audio: String,
    seconds: usize,
    threads: usize,
    repeats: usize,
    from_file: bool,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let number = |i: usize, default: usize| -> Result<usize, String> {
        match raw.get(i) {
            None => Ok(default),
            Some(v) => match v.parse::<usize>() {
                Ok(n) if n > 0 => Ok(n),
                _ => Err(format!("{v}: нужно целое больше нуля")),
            },
        }
    };
    if raw.len() < 2 || raw.len() > 6 {
        return Err(USAGE.into());
    }
    let from_file = match raw.get(5).map(String::as_str) {
        None | Some("bytes") => false,
        Some("file") => true,
        Some(other) => return Err(format!("{other}: нужно bytes или file")),
    };
    Ok(Args {
        model: raw[0].clone(),
        audio: raw[1].clone(),
        seconds: number(2, DEFAULT_SECONDS)?,
        threads: number(3, DEFAULT_THREADS)?,
        repeats: number(4, DEFAULT_REPEATS)?,
        from_file,
    })
}

fn read_f32le(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err("длина файла звука не кратна 4 байтам".into());
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

/// Строка `VmHWM` или `VmRSS` из /proc/self/status в мегабайтах; на других ОС None.
fn memory_mb(field: &str) -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with(field))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

fn memory_line() -> String {
    match (memory_mb("VmHWM:"), memory_mb("VmRSS:")) {
        (Some(peak), Some(now)) => format!("память: пик {peak:.0} МБ, сейчас {now:.0} МБ"),
        _ => "память: /proc/self/status недоступен".into(),
    }
}

fn load(args: &Args) -> Result<AgeGender, String> {
    let started = Instant::now();
    if args.from_file {
        let model = AgeGender::load_pool(Path::new(&args.model), args.threads, 1, WINDOW_SEC)
            .map_err(|e| e.to_string())?;
        println!(
            "модель с диска: сессия за {:.1} с, потоков {}",
            started.elapsed().as_secs_f64(),
            args.threads
        );
        println!("{}", memory_line());
        return Ok(model);
    }
    let bytes = std::fs::read(&args.model).map_err(|e| format!("{}: {e}", args.model))?;
    let read_sec = started.elapsed().as_secs_f64();
    let loading = Instant::now();
    let model =
        AgeGender::from_bytes(&bytes, args.threads, WINDOW_SEC).map_err(|e| e.to_string())?;
    println!(
        "модель {:.0} МБ из байтов: чтение {read_sec:.1} с, сессия {:.1} с, потоков {}",
        bytes.len() as f64 / 1e6,
        loading.elapsed().as_secs_f64(),
        args.threads
    );
    println!("{}", memory_line());
    // Байты модели после создания сессии не нужны, как и в приложении.
    drop(bytes);
    println!("после освобождения байтов модели: {}", memory_line());
    Ok(model)
}

fn run(args: &Args) -> Result<(), String> {
    let model = load(args)?;

    let audio = std::fs::read(&args.audio).map_err(|e| format!("{}: {e}", args.audio))?;
    let samples = read_f32le(&audio)?;
    let need = args.seconds * SAMPLE_RATE;
    if samples.len() < need {
        return Err(format!(
            "в записи {:.1} с, нужно {} с",
            samples.len() as f64 / SAMPLE_RATE as f64,
            args.seconds
        ));
    }
    let piece = (WINDOW_SEC as usize) * SAMPLE_RATE;
    let pieces: Vec<&[f32]> = samples[..need].chunks(piece).collect();
    for round in 1..=args.repeats {
        let t = Instant::now();
        let mut child = Vec::new();
        for p in &pieces {
            if let Some(a) = model.analyze(p).map_err(|e| e.to_string())? {
                child.push(a.traits.child);
            }
        }
        let sec = t.elapsed().as_secs_f64();
        println!(
            "повтор {round}: {} с речи, {} кусков, {sec:.1} с, в {:.1} раза быстрее звука, \
             оценка «ребёнок» по кускам {child:.2?}",
            args.seconds,
            pieces.len(),
            args.seconds as f64 / sec
        );
    }
    println!("в конце: {}", memory_line());
    Ok(())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ошибка: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(&list.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn defaults_are_thirty_seconds_four_threads_and_three_repeats() {
        let a = args(&["m.onnx", "a.f32"]).unwrap();
        assert_eq!(
            (a.seconds, a.threads, a.repeats, a.from_file),
            (30, 4, 3, false)
        );
        let a = args(&["m.onnx", "a.f32", "20", "2", "1"]).unwrap();
        assert_eq!((a.seconds, a.threads, a.repeats), (20, 2, 1));
    }

    #[test]
    fn missing_files_or_bad_numbers_are_errors() {
        assert!(args(&["m.onnx"]).is_err());
        assert!(args(&["m.onnx", "a.f32", "0"]).is_err());
        assert!(args(&["m.onnx", "a.f32", "x"]).is_err());
        assert!(args(&["m.onnx", "a.f32", "30", "4", "3", "disk"]).is_err());
    }

    #[test]
    fn the_model_can_be_loaded_from_the_file_itself() {
        let a = args(&["m.onnx", "a.f32", "30", "4", "3", "file"]).unwrap();
        assert!(a.from_file);
    }

    #[test]
    fn raw_float_samples_are_read_little_endian() {
        let bytes: Vec<u8> = [0.5_f32, -1.0]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        assert_eq!(read_f32le(&bytes).unwrap(), [0.5, -1.0]);
        assert!(read_f32le(&bytes[..5]).is_err());
    }
}
