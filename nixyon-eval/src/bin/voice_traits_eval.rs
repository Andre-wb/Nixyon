//! Замер пола и возраста по голосу моделью audEERING на наборах с разметкой говорящих:
//! `voice_traits_eval --out <папка отчёта> [--sps <папка>] [--speechocean <папка>]
//! [--ort-models <папка>] [--jobs <число>] [--threads <число>] [--window-sec <с>]`.
//! `--sps`: набор Common Voice Spontaneous Speech (таблица `ss-corpus-*.tsv` и `audios/`),
//! `--speechocean`: speechocean762 (`train/`, `test/`, `WAVE/`); можно оба и по нескольку.
//! Каждый клип приводится к 16 кГц тем же путём, что импорт в приложении. Клипы без пола
//! и без возраста в разметке пропускаются. В папку отчёта пишутся `report.md` (он же
//! печатается) и `summary.json`. `--jobs` число параллельных сессий модели (по умолчанию
//! ядра, делённые на `--threads`), `--threads` потоки одной сессии (по умолчанию 2),
//! `--window-sec` окно, на которое режется длинный клип (по умолчанию 10 с).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use nixyon_audio::{TARGET_SAMPLE_RATE, WavReader};
use nixyon_eval::{
    LabeledVoiceClip, ScoredVoiceClip, VoiceTraitsReport, clip_to_wav16k, read_speechocean,
    read_sps_corpus, render_voice_traits,
};
use nixyon_ort::{AgeGender, OrtManifest, OrtRole};
use serde::Serialize;

const USAGE: &str = "использование: voice_traits_eval --out <папка отчёта> [--sps <папка>] \
     [--speechocean <папка>] [--ort-models <папка>] [--jobs <число>] [--threads <число>] \
     [--window-sec <с>]";
const DEFAULT_ORT_MODELS: &str = "models/ort";
const DEFAULT_THREADS: usize = 2;
const DEFAULT_WINDOW_SEC: f64 = 10.0;
const WINDOW_RANGE: std::ops::RangeInclusive<f64> = 1.0..=60.0;
const READ_FRAMES: usize = 1 << 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetKind {
    Sps,
    Speechocean,
}

#[derive(Debug, PartialEq)]
struct Args {
    out: PathBuf,
    sets: Vec<(SetKind, PathBuf)>,
    ort_models: PathBuf,
    jobs: Option<usize>,
    threads: usize,
    window_sec: f64,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut out = None;
    let mut sets = Vec::new();
    let mut ort_models = PathBuf::from(DEFAULT_ORT_MODELS);
    let mut jobs = None;
    let mut threads = DEFAULT_THREADS;
    let mut window_sec = DEFAULT_WINDOW_SEC;
    let mut it = raw.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag}: нет значения"));
        match flag.as_str() {
            "--out" => out = Some(PathBuf::from(value()?)),
            "--sps" => sets.push((SetKind::Sps, PathBuf::from(value()?))),
            "--speechocean" => sets.push((SetKind::Speechocean, PathBuf::from(value()?))),
            "--ort-models" => ort_models = PathBuf::from(value()?),
            "--jobs" => jobs = Some(positive(flag, value()?)?),
            "--threads" => threads = positive(flag, value()?)?,
            "--window-sec" => {
                let v: f64 = value()?.parse().map_err(|_| format!("{flag}: не число"))?;
                if !WINDOW_RANGE.contains(&v) {
                    return Err(format!("{flag}: от 1 до 60 с"));
                }
                window_sec = v;
            }
            other => return Err(format!("неизвестный флаг {other}")),
        }
    }
    let out = out.ok_or("нет --out")?;
    if sets.is_empty() {
        return Err("нужен хотя бы один набор: --sps или --speechocean".into());
    }
    Ok(Args {
        out,
        sets,
        ort_models,
        jobs,
        threads,
        window_sec,
    })
}

fn positive(flag: &str, value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("{flag}: целое больше нуля")),
    }
}

fn set_name(dir: &Path) -> String {
    dir.file_name().map_or_else(
        || dir.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn read_set(kind: SetKind, dir: &Path) -> Result<Vec<LabeledVoiceClip>, String> {
    let clips = match kind {
        SetKind::Sps => read_sps_corpus(dir).map_err(|e| e.to_string())?,
        SetKind::Speechocean => read_speechocean(dir).map_err(|e| e.to_string())?,
    };
    Ok(clips
        .into_iter()
        .filter(|c| c.gender.is_some() || c.child.is_some())
        .collect())
}

fn read_wav16k(path: &Path) -> Result<Vec<f32>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("wav: {e}"))?;
    let mut reader = WavReader::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
    if reader.format().sample_rate != TARGET_SAMPLE_RATE {
        return Err(format!("wav не {TARGET_SAMPLE_RATE} Гц"));
    }
    let mut samples = Vec::new();
    while reader
        .read_mono(READ_FRAMES, &mut samples)
        .map_err(|e| e.to_string())?
        > 0
    {}
    Ok(samples)
}

#[derive(Serialize)]
struct Skipped {
    audio: String,
    reason: String,
}

#[derive(Serialize)]
struct SetSummary<'a> {
    name: &'a str,
    report: &'a VoiceTraitsReport,
}

#[derive(Serialize)]
struct Summary<'a> {
    models_version: &'a str,
    window_sec: f64,
    jobs: usize,
    threads: usize,
    audio_sec: f64,
    wall_sec: f64,
    /// Время модели на секунду звука, сумма по всем сессиям.
    model_ms_per_audio_sec: f64,
    sets: Vec<SetSummary<'a>>,
    skipped: Vec<Skipped>,
}

struct Outcome {
    set: usize,
    clip: ScoredVoiceClip,
    model_time: Duration,
}

fn score_clip(
    model: &AgeGender,
    clip: &LabeledVoiceClip,
    wav: &Path,
) -> Result<Option<(ScoredVoiceClip, Duration)>, String> {
    clip_to_wav16k(&clip.audio, wav)?;
    let samples = read_wav16k(wav)?;
    let started = Instant::now();
    let traits = model.traits(&samples).map_err(|e| e.to_string())?;
    let model_time = started.elapsed();
    Ok(traits.map(|traits| {
        (
            ScoredVoiceClip {
                speaker: clip.speaker.clone(),
                gender: clip.gender,
                child: clip.child,
                duration_sec: samples.len() as f64 / f64::from(TARGET_SAMPLE_RATE),
                traits,
            },
            model_time,
        )
    }))
}

fn run(args: &Args) -> Result<String, String> {
    let manifest = OrtManifest::load(&args.ort_models).map_err(|e| e.to_string())?;
    let mut sets = Vec::new();
    for (kind, dir) in &args.sets {
        sets.push((set_name(dir), read_set(*kind, dir)?));
    }
    let work: Vec<(usize, &LabeledVoiceClip)> = sets
        .iter()
        .enumerate()
        .flat_map(|(i, (_, clips))| clips.iter().map(move |c| (i, c)))
        .collect();
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let jobs = args.jobs.unwrap_or((cores / args.threads).max(1));
    let model = AgeGender::load_pool(
        &manifest
            .path(OrtRole::AgeGender)
            .map_err(|e| e.to_string())?,
        args.threads,
        jobs,
        args.window_sec,
    )
    .map_err(|e| e.to_string())?;
    let tmp = std::env::temp_dir().join(format!("nixyon-voice-traits-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let outcomes = Mutex::new(Vec::new());
    let skipped = Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for worker in 0..jobs {
            let (work, model, next, done, outcomes, skipped) =
                (&work, &model, &next, &done, &outcomes, &skipped);
            let wav = tmp.join(format!("{worker}.wav"));
            scope.spawn(move || {
                while let Some(&(set, clip)) = work.get(next.fetch_add(1, Ordering::Relaxed)) {
                    match score_clip(model, clip, &wav) {
                        Ok(Some((clip, model_time))) => {
                            if let Ok(mut o) = outcomes.lock() {
                                o.push(Outcome {
                                    set,
                                    clip,
                                    model_time,
                                });
                            }
                        }
                        result => {
                            let reason = match result {
                                Err(e) => e,
                                _ => "кусок короче 25 мс".into(),
                            };
                            if let Ok(mut s) = skipped.lock() {
                                s.push(Skipped {
                                    audio: clip.audio.display().to_string(),
                                    reason,
                                });
                            }
                        }
                    }
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if n % 100 == 0 || n == work.len() {
                        eprintln!("клипов {n} из {}", work.len());
                    }
                }
            });
        }
    });
    let wall_sec = started.elapsed().as_secs_f64();
    let _ = std::fs::remove_dir_all(&tmp);
    let outcomes = outcomes.into_inner().map_err(|_| "сбой потока замера")?;
    let mut skipped = skipped.into_inner().map_err(|_| "сбой потока замера")?;
    skipped.sort_by(|a, b| a.audio.cmp(&b.audio));
    let audio_sec: f64 = outcomes.iter().map(|o| o.clip.duration_sec).sum();
    let model_sec: f64 = outcomes.iter().map(|o| o.model_time.as_secs_f64()).sum();
    let reports: Vec<(String, VoiceTraitsReport)> = sets
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            let clips: Vec<ScoredVoiceClip> = outcomes
                .iter()
                .filter(|o| o.set == i)
                .map(|o| o.clip.clone())
                .collect();
            (name.clone(), VoiceTraitsReport::new(&clips))
        })
        .collect();
    let model_ms_per_audio_sec = if audio_sec > 0.0 {
        1000.0 * model_sec / audio_sec
    } else {
        0.0
    };
    let mut report = render_voice_traits(&reports);
    report.push_str(&format!(
        "\nМодель: {:.1} мс на секунду звука (сумма по {jobs} сессиям по {} потока), \
         окно {} с, всего {:.0} с. Пропущено клипов: {}.\n",
        model_ms_per_audio_sec,
        args.threads,
        args.window_sec,
        wall_sec,
        skipped.len()
    ));
    let summary = Summary {
        models_version: manifest.version(),
        window_sec: args.window_sec,
        jobs,
        threads: args.threads,
        audio_sec,
        wall_sec,
        model_ms_per_audio_sec,
        sets: reports
            .iter()
            .map(|(name, report)| SetSummary { name, report })
            .collect(),
        skipped,
    };
    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    let json = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    std::fs::write(args.out.join("summary.json"), json).map_err(|e| e.to_string())?;
    std::fs::write(args.out.join("report.md"), &report).map_err(|e| e.to_string())?;
    Ok(report)
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&raw) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
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
    fn sets_and_settings_are_read_from_flags() {
        let a = args(&[
            "--out",
            "r",
            "--sps",
            "s",
            "--speechocean",
            "o",
            "--jobs",
            "3",
            "--threads",
            "1",
            "--window-sec",
            "6",
        ])
        .unwrap();
        assert_eq!(a.out, PathBuf::from("r"));
        assert_eq!(
            a.sets,
            [
                (SetKind::Sps, PathBuf::from("s")),
                (SetKind::Speechocean, PathBuf::from("o"))
            ]
        );
        assert_eq!((a.jobs, a.threads, a.window_sec), (Some(3), 1, 6.0));
        assert_eq!(a.ort_models, PathBuf::from(DEFAULT_ORT_MODELS));
    }

    #[test]
    fn defaults_apply_when_settings_are_not_given() {
        let a = args(&["--out", "r", "--sps", "s"]).unwrap();
        assert_eq!(a.jobs, None);
        assert_eq!(a.threads, DEFAULT_THREADS);
        assert_eq!(a.window_sec, DEFAULT_WINDOW_SEC);
    }

    #[test]
    fn missing_out_or_sets_and_bad_values_are_errors() {
        assert!(args(&["--sps", "s"]).is_err());
        assert!(args(&["--out", "r"]).is_err());
        assert!(args(&["--out", "r", "--sps", "s", "--jobs", "0"]).is_err());
        assert!(args(&["--out", "r", "--sps", "s", "--window-sec", "0.5"]).is_err());
        assert!(args(&["--out", "r", "--sps", "s", "--what"]).is_err());
        assert!(args(&["--out"]).is_err());
    }

    #[test]
    fn the_set_name_comes_from_its_folder() {
        assert_eq!(
            set_name(Path::new("/m/sps-corpus-5.0-ru")),
            "sps-corpus-5.0-ru"
        );
    }
}
