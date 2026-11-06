//! Обучение и замер классификатора «ребёнок или взрослый» на признаках двух моделей:
//! эмбеддинге ReDimNet2-B6 и `hidden_states` audEERING.
//! `child_adult_eval --out <папка> (--speechocean <папка> [--samromur <папка>]...
//! [--sps <папка>]... [--childmandarin <папка>]... [--save-features] | --features <features.json>)
//! [--ort-models <папка>] [--jobs <число>] [--threads <число>]
//! [--max-clips-per-speaker <число>] [--max-speakers-per-class <число>]`.
//! Обучение на train speechocean762 и train наборов `--samromur` (Samrómur и Samrómur
//! Children), проверка на их test и dev (говорящие другие) и на остальных наборах: `--sps`
//! Common Voice Spontaneous Speech (взрослые), `--childmandarin` распакованные части
//! ChildMandarin (дети 3-5 лет). Подростки 13-18 лет не участвуют. С каждого говорящего
//! берётся не больше `--max-clips-per-speaker` клипов (по умолчанию 30), из каждой части
//! набора не больше `--max-speakers-per-class` детей и столько же взрослых (по умолчанию
//! 250, первые по id), в порядке набора. В stderr идут строки `@progress` для хаба: доля
//! считается по клипам. С `--save-features` признаки сохраняются в `<out>/features.json`
//! (сотни мегабайт на полный набор, поэтому только по просьбе); `--features` берёт их из
//! готового файла без моделей. В папку пишутся `report.md` (он же печатается),
//! `summary.json` и веса `child_adult_<признаки>.json`. `--jobs` число параллельных
//! сессий моделей (по умолчанию ядра, делённые на `--threads`), `--threads` потоки одной
//! сессии (по умолчанию 2).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use nixyon_audio::{TARGET_SAMPLE_RATE, WavReader};
use nixyon_core::VoiceEmbedder;
use nixyon_eval::{
    ChildAdultRow, FeatureClip, FeatureRole, LabeledVoiceClip, MAX_ADULTS_AS_CHILD, ProgressMeter,
    TrainedChildAdult, clip_to_wav16k, evaluate_child_adult, read_childmandarin, read_samromur,
    read_speechocean_split, read_sps_corpus, render_child_adult, train_child_adult,
};
use nixyon_ort::{AgeGender, ChildAdultFeatures, OrtManifest, OrtRole, Redimnet2};
use serde::Serialize;

const USAGE: &str = "использование: child_adult_eval --out <папка> (--speechocean <папка> \
     [--samromur <папка>]... [--sps <папка>]... [--childmandarin <папка>]... [--save-features] | \
     --features <features.json>) [--ort-models <папка>] [--jobs <число>] \
     [--threads <число>] [--max-clips-per-speaker <число>] [--max-speakers-per-class <число>]";
const DEFAULT_ORT_MODELS: &str = "models/ort";
const DEFAULT_THREADS: usize = 2;
const DEFAULT_MAX_CLIPS: usize = 30;
const DEFAULT_MAX_SPEAKERS: usize = 250;
/// Окно audEERING, как в замере пола и возраста.
const AGE_GENDER_WINDOW_SEC: f64 = 10.0;
const READ_FRAMES: usize = 1 << 16;
const FEATURES_FILE: &str = "features.json";

#[derive(Debug, PartialEq)]
enum Source {
    Datasets {
        speechocean: PathBuf,
        samromur: Vec<PathBuf>,
        sps: Vec<PathBuf>,
        childmandarin: Vec<PathBuf>,
        save_features: bool,
    },
    Features(PathBuf),
}

#[derive(Debug, PartialEq)]
struct Args {
    out: PathBuf,
    source: Source,
    ort_models: PathBuf,
    jobs: Option<usize>,
    threads: usize,
    max_clips: usize,
    max_speakers: usize,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut out = None;
    let mut speechocean = None;
    let mut samromur = Vec::new();
    let mut sps = Vec::new();
    let mut childmandarin = Vec::new();
    let mut features = None;
    let mut save_features = false;
    let mut ort_models = PathBuf::from(DEFAULT_ORT_MODELS);
    let mut jobs = None;
    let mut threads = DEFAULT_THREADS;
    let mut max_clips = DEFAULT_MAX_CLIPS;
    let mut max_speakers = DEFAULT_MAX_SPEAKERS;
    let mut it = raw.iter();
    while let Some(flag) = it.next() {
        if flag == "--save-features" {
            save_features = true;
            continue;
        }
        let mut value = || it.next().ok_or_else(|| format!("{flag}: нет значения"));
        match flag.as_str() {
            "--out" => out = Some(PathBuf::from(value()?)),
            "--speechocean" => speechocean = Some(PathBuf::from(value()?)),
            "--samromur" => samromur.push(PathBuf::from(value()?)),
            "--sps" => sps.push(PathBuf::from(value()?)),
            "--childmandarin" => childmandarin.push(PathBuf::from(value()?)),
            "--features" => features = Some(PathBuf::from(value()?)),
            "--ort-models" => ort_models = PathBuf::from(value()?),
            "--jobs" => jobs = Some(positive(flag, value()?)?),
            "--threads" => threads = positive(flag, value()?)?,
            "--max-clips-per-speaker" => max_clips = positive(flag, value()?)?,
            "--max-speakers-per-class" => max_speakers = positive(flag, value()?)?,
            other => return Err(format!("неизвестный флаг {other}")),
        }
    }
    let out = out.ok_or("нет --out")?;
    let source = match (features, speechocean) {
        (Some(_), Some(_)) => return Err("--features и --speechocean вместе нельзя".into()),
        (Some(_), None) if save_features => {
            return Err("--save-features только с наборами".into());
        }
        (Some(file), None) if samromur.is_empty() && sps.is_empty() && childmandarin.is_empty() => {
            Source::Features(file)
        }
        (Some(_), None) => return Err("с --features наборы не указываются".into()),
        (None, Some(speechocean)) => Source::Datasets {
            speechocean,
            samromur,
            sps,
            childmandarin,
            save_features,
        },
        (None, None) => return Err("нужен --speechocean или --features".into()),
    };
    Ok(Args {
        out,
        source,
        ort_models,
        jobs,
        threads,
        max_clips,
        max_speakers,
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

/// Клип к обработке: набор, роль и разметка.
struct Job {
    role: FeatureRole,
    set: String,
    clip: LabeledVoiceClip,
}

/// Сколько брать из части набора: клипов на говорящего и говорящих каждого класса.
#[derive(Debug, Clone, Copy)]
struct Limits {
    clips: usize,
    speakers: usize,
}

/// Клипы с разметкой «ребёнок или взрослый»: не больше `limits.speakers` говорящих каждого
/// класса (первые по id) и не больше `limits.clips` клипов на говорящего.
fn take(role: FeatureRole, set: String, clips: Vec<LabeledVoiceClip>, limits: Limits) -> Vec<Job> {
    let mut speakers: BTreeMap<(bool, &str), ()> = BTreeMap::new();
    for c in &clips {
        if let Some(child) = c.child {
            speakers.insert((child, c.speaker.as_str()), ());
        }
    }
    let mut kept: HashSet<String> = HashSet::new();
    for class in [true, false] {
        kept.extend(
            speakers
                .keys()
                .filter(|(c, _)| *c == class)
                .take(limits.speakers)
                .map(|(_, s)| s.to_string()),
        );
    }
    let mut per_speaker: HashMap<String, usize> = HashMap::new();
    clips
        .into_iter()
        .filter(|c| c.child.is_some() && kept.contains(&c.speaker))
        .filter(|c| {
            let n = per_speaker.entry(c.speaker.clone()).or_default();
            *n += 1;
            *n <= limits.clips
        })
        .map(|clip| Job {
            role,
            set: set.clone(),
            clip,
        })
        .collect()
}

fn read_jobs(
    speechocean: &Path,
    samromur: &[PathBuf],
    sps: &[PathBuf],
    childmandarin: &[PathBuf],
    limits: Limits,
) -> Result<Vec<Job>, String> {
    let name = set_name(speechocean);
    let mut jobs = Vec::new();
    for (role, split) in [(FeatureRole::Train, "train"), (FeatureRole::Test, "test")] {
        let clips = read_speechocean_split(speechocean, split).map_err(|e| e.to_string())?;
        jobs.extend(take(role, format!("{name}/{split}"), clips, limits));
    }
    for dir in samromur {
        let mut by_split: BTreeMap<String, Vec<LabeledVoiceClip>> = BTreeMap::new();
        for c in read_samromur(dir).map_err(|e| e.to_string())? {
            by_split.entry(c.split).or_default().push(c.clip);
        }
        for (split, clips) in by_split {
            let role = if split == "train" {
                FeatureRole::Train
            } else {
                FeatureRole::Test
            };
            jobs.extend(take(
                role,
                format!("{}/{split}", set_name(dir)),
                clips,
                limits,
            ));
        }
    }
    for dir in sps {
        let clips = read_sps_corpus(dir).map_err(|e| e.to_string())?;
        jobs.extend(take(FeatureRole::Extra, set_name(dir), clips, limits));
    }
    for dir in childmandarin {
        let clips = read_childmandarin(dir).map_err(|e| e.to_string())?;
        jobs.extend(take(FeatureRole::Extra, set_name(dir), clips, limits));
    }
    Ok(jobs)
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

struct Models {
    age_gender: AgeGender,
    redimnet: Redimnet2,
}

fn featurize(models: &Models, job: &Job, wav: &Path) -> Result<Option<FeatureClip>, String> {
    clip_to_wav16k(&job.clip.audio, wav)?;
    let samples = read_wav16k(wav)?;
    let Some(child) = job.clip.child else {
        return Ok(None);
    };
    let Some(analysis) = models
        .age_gender
        .analyze(&samples)
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let embedding = models.redimnet.embed(&samples).map_err(|e| e.to_string())?;
    if embedding.is_empty() {
        return Ok(None);
    }
    Ok(Some(FeatureClip {
        role: job.role,
        set: job.set.clone(),
        speaker: job.clip.speaker.clone(),
        child,
        duration_sec: samples.len() as f64 / f64::from(TARGET_SAMPLE_RATE),
        redimnet2: embedding,
        age_gender_hidden: analysis.hidden_states,
    }))
}

/// Части наборов, у которых были клипы к обработке, но не прочитался ни один. Так выглядит
/// неподдержанный формат звука или испорченный набор: замер без этой части молча дал бы
/// другие цифры.
fn empty_sets(jobs: &[Job], clips: &[FeatureClip]) -> Vec<String> {
    let read: HashSet<&str> = clips.iter().map(|c| c.set.as_str()).collect();
    let mut empty: Vec<String> = jobs
        .iter()
        .map(|j| j.set.as_str())
        .filter(|set| !read.contains(set))
        .map(str::to_string)
        .collect();
    empty.sort();
    empty.dedup();
    empty
}

#[derive(Serialize)]
struct Skipped {
    set: String,
    audio: String,
    reason: String,
}

fn extract(
    args: &Args,
    jobs_list: &[Job],
    jobs: usize,
) -> Result<(Vec<FeatureClip>, Vec<Skipped>, String), String> {
    let manifest = OrtManifest::load(&args.ort_models).map_err(|e| e.to_string())?;
    let path = |role| manifest.path(role).map_err(|e| e.to_string());
    let models = Models {
        age_gender: AgeGender::load_pool(
            &path(OrtRole::AgeGender)?,
            args.threads,
            jobs,
            AGE_GENDER_WINDOW_SEC,
        )
        .map_err(|e| e.to_string())?,
        redimnet: Redimnet2::load_pool(&path(OrtRole::Redimnet2)?, args.threads, jobs)
            .map_err(|e| e.to_string())?,
    };
    let tmp = std::env::temp_dir().join(format!("nixyon-child-adult-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let clips = Mutex::new(Vec::new());
    let skipped = Mutex::new(Vec::new());
    // Доля по клипам: длительность клипа до чтения звука не известна.
    let meter = Mutex::new(ProgressMeter::new(jobs_list.len() as f64, jobs_list.len()));
    let started = Instant::now();
    std::thread::scope(|scope| {
        for worker in 0..jobs {
            let (models, next, done, clips, skipped, meter, started) =
                (&models, &next, &done, &clips, &skipped, &meter, &started);
            let wav = tmp.join(format!("{worker}.wav"));
            scope.spawn(move || {
                while let Some(job) = jobs_list.get(next.fetch_add(1, Ordering::Relaxed)) {
                    match featurize(models, job, &wav) {
                        Ok(Some(clip)) => {
                            if let Ok(mut c) = clips.lock() {
                                c.push(clip);
                            }
                        }
                        result => {
                            let reason = match result {
                                Err(e) => e,
                                _ => "кусок короче 25 мс".into(),
                            };
                            if let Ok(mut s) = skipped.lock() {
                                s.push(Skipped {
                                    set: job.set.clone(),
                                    audio: job.clip.audio.display().to_string(),
                                    reason,
                                });
                            }
                        }
                    }
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if n % 200 == 0 || n == jobs_list.len() {
                        eprintln!("клипов {n} из {}", jobs_list.len());
                    }
                    if let Ok(mut m) = meter.lock()
                        && let Some(line) = m.clip_done(1.0, started.elapsed())
                    {
                        eprintln!("{line}");
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&tmp);
    let mut clips = clips.into_inner().map_err(|_| "сбой потока признаков")?;
    // Потоки заканчивают клипы в любом порядке; обучение и отчёт не должны от него зависеть.
    clips.sort_by(|a, b| {
        (a.role, &a.set, &a.speaker)
            .cmp(&(b.role, &b.set, &b.speaker))
            .then(a.duration_sec.total_cmp(&b.duration_sec))
    });
    let mut skipped = skipped.into_inner().map_err(|_| "сбой потока признаков")?;
    skipped.sort_by(|a, b| a.audio.cmp(&b.audio));
    Ok((clips, skipped, manifest.version().to_string()))
}

#[derive(Serialize)]
struct FeatureResult<'a> {
    features: &'static str,
    l2: f64,
    l2_losses: &'a [f64],
    confidence: f32,
    confidence_found: bool,
    rows: &'a [ChildAdultRow],
}

#[derive(Serialize)]
struct Summary<'a> {
    models_version: Option<&'a str>,
    clips: usize,
    max_clips_per_speaker: usize,
    max_speakers_per_class: usize,
    max_adults_as_child: f64,
    feature_sec: Option<f64>,
    results: Vec<FeatureResult<'a>>,
    skipped: &'a [Skipped],
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn run(args: &Args) -> Result<String, String> {
    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let (clips, skipped, version, feature_sec) = match &args.source {
        Source::Features(file) => {
            let text =
                std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
            let clips: Vec<FeatureClip> =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
            (clips, Vec::new(), None, None)
        }
        Source::Datasets {
            speechocean,
            samromur,
            sps,
            childmandarin,
            save_features,
        } => {
            let limits = Limits {
                clips: args.max_clips,
                speakers: args.max_speakers,
            };
            let list = read_jobs(speechocean, samromur, sps, childmandarin, limits)?;
            let jobs = args.jobs.unwrap_or((cores / args.threads).max(1));
            let started = Instant::now();
            let (clips, skipped, version) = extract(args, &list, jobs)?;
            let empty = empty_sets(&list, &clips);
            if !empty.is_empty() {
                let reasons: Vec<String> = empty
                    .iter()
                    .map(|set| {
                        let reason = skipped
                            .iter()
                            .find(|s| &s.set == set)
                            .map_or("нет", |s| s.reason.as_str());
                        format!("{set} ({reason})")
                    })
                    .collect();
                return Err(format!(
                    "не прочитан ни один клип в частях наборов: {}",
                    reasons.join("; ")
                ));
            }
            let sec = started.elapsed().as_secs_f64();
            if *save_features {
                let json = serde_json::to_string(&clips).map_err(|e| e.to_string())?;
                write(&args.out.join(FEATURES_FILE), &json)?;
            }
            (clips, skipped, Some(version), Some(sec))
        }
    };
    let train: Vec<&FeatureClip> = clips
        .iter()
        .filter(|c| c.role == FeatureRole::Train)
        .collect();
    let mut results: Vec<(TrainedChildAdult, Vec<ChildAdultRow>)> = Vec::new();
    for kind in ChildAdultFeatures::ALL {
        let trained = train_child_adult(kind, &train, MAX_ADULTS_AS_CHILD, cores)?;
        let rows = evaluate_child_adult(&trained, &clips)?;
        let json = trained.model.to_json().map_err(|e| e.to_string())?;
        write(
            &args.out.join(format!("child_adult_{}.json", kind.key())),
            &json,
        )?;
        results.push((trained, rows));
    }
    let mut report = render_child_adult(&results);
    report.push_str(&format!(
        "\nКлипов {}, не больше {} на говорящего и {} говорящих каждого класса в части набора, \
         пропущено {}.{}\n",
        clips.len(),
        args.max_clips,
        args.max_speakers,
        skipped.len(),
        feature_sec.map_or(String::new(), |s| format!(
            " Признаки посчитаны за {s:.0} с."
        ))
    ));
    let summary = Summary {
        models_version: version.as_deref(),
        clips: clips.len(),
        max_clips_per_speaker: args.max_clips,
        max_speakers_per_class: args.max_speakers,
        max_adults_as_child: MAX_ADULTS_AS_CHILD,
        feature_sec,
        results: results
            .iter()
            .map(|(t, rows)| FeatureResult {
                features: t.model.features.key(),
                l2: t.l2,
                l2_losses: &t.l2_losses,
                confidence: t.model.confidence,
                confidence_found: t.confidence_found,
                rows,
            })
            .collect(),
        skipped: &skipped,
    };
    let json = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    write(&args.out.join("summary.json"), &json)?;
    write(&args.out.join("report.md"), &report)?;
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
    fn datasets_and_settings_are_read_from_flags() {
        let a = args(&[
            "--out",
            "r",
            "--speechocean",
            "o",
            "--samromur",
            "k",
            "--sps",
            "s",
            "--childmandarin",
            "c",
            "--jobs",
            "3",
            "--threads",
            "1",
            "--max-clips-per-speaker",
            "5",
            "--max-speakers-per-class",
            "7",
        ])
        .unwrap();
        assert_eq!(
            a.source,
            Source::Datasets {
                speechocean: "o".into(),
                samromur: vec!["k".into()],
                sps: vec!["s".into()],
                childmandarin: vec!["c".into()],
                save_features: false,
            }
        );
        assert_eq!(
            (a.jobs, a.threads, a.max_clips, a.max_speakers),
            (Some(3), 1, 5, 7)
        );
    }

    #[test]
    fn features_are_saved_only_when_asked() {
        let a = args(&["--out", "r", "--speechocean", "o", "--save-features"]).unwrap();
        assert!(matches!(
            a.source,
            Source::Datasets {
                save_features: true,
                ..
            }
        ));
        let a = args(&["--out", "r", "--speechocean", "o"]).unwrap();
        assert!(matches!(
            a.source,
            Source::Datasets {
                save_features: false,
                ..
            }
        ));
    }

    #[test]
    fn saving_features_read_from_a_file_is_an_error() {
        assert!(args(&["--out", "r", "--features", "f", "--save-features"]).is_err());
    }

    #[test]
    fn ready_features_replace_the_datasets() {
        let a = args(&["--out", "r", "--features", "f.json"]).unwrap();
        assert_eq!(a.source, Source::Features("f.json".into()));
        assert_eq!(a.max_clips, DEFAULT_MAX_CLIPS);
        assert_eq!(a.max_speakers, DEFAULT_MAX_SPEAKERS);
    }

    #[test]
    fn conflicting_or_missing_sources_and_bad_values_are_errors() {
        assert!(args(&["--out", "r"]).is_err());
        assert!(args(&["--speechocean", "o"]).is_err());
        assert!(args(&["--out", "r", "--features", "f", "--speechocean", "o"]).is_err());
        assert!(args(&["--out", "r", "--features", "f", "--sps", "s"]).is_err());
        assert!(args(&["--out", "r", "--features", "f", "--samromur", "k"]).is_err());
        assert!(args(&["--out", "r", "--speechocean", "o", "--jobs", "0"]).is_err());
        assert!(args(&["--out", "r", "--speechocean", "o", "--what"]).is_err());
    }

    fn feature_clip(set: &str) -> FeatureClip {
        FeatureClip {
            role: FeatureRole::Test,
            set: set.into(),
            speaker: "a".into(),
            child: true,
            duration_sec: 1.0,
            redimnet2: vec![0.0],
            age_gender_hidden: vec![0.0],
        }
    }

    fn job(set: &str) -> Job {
        Job {
            role: FeatureRole::Test,
            set: set.into(),
            clip: LabeledVoiceClip {
                speaker: "a".into(),
                audio: "a.flac".into(),
                gender: None,
                child: Some(true),
            },
        }
    }

    #[test]
    fn a_set_part_without_a_single_read_clip_is_reported() {
        let jobs = [job("samromur/dev"), job("samromur/dev"), job("so/test")];
        let clips = [feature_clip("so/test")];
        assert_eq!(empty_sets(&jobs, &clips), ["samromur/dev"]);
    }

    #[test]
    fn a_set_part_with_some_read_clips_is_not_reported() {
        let jobs = [job("so/test"), job("so/test")];
        assert!(empty_sets(&jobs, &[feature_clip("so/test")]).is_empty());
    }

    #[test]
    fn only_labeled_clips_are_taken_and_at_most_the_limits() {
        let clip = |speaker: &str, child| LabeledVoiceClip {
            speaker: speaker.into(),
            audio: "a.wav".into(),
            gender: None,
            child,
        };
        let jobs = take(
            FeatureRole::Test,
            "s".into(),
            vec![
                clip("a", Some(true)),
                clip("a", Some(true)),
                clip("a", Some(true)),
                clip("b", None),
                clip("d", Some(true)),
                clip("c", Some(false)),
            ],
            Limits {
                clips: 2,
                speakers: 1,
            },
        );
        let speakers: Vec<&str> = jobs.iter().map(|j| j.clip.speaker.as_str()).collect();
        assert_eq!(speakers, ["a", "a", "c"]);
    }
}
