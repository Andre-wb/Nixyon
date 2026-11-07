//! Качество распознавания на наборах записей:
//! `quality_eval <папка моделей> <корень наборов> --out <папка отчёта>
//! --take <набор>=<минуты|all> [--take ...] [--jobs <число>] [--engine-threads <число>]`.
//! Набор: `<корень>/<набор>/dataset.json` и `clips.jsonl` (scripts/build-dataset-index.py).
//! Из каждого набора берётся столько минут звука, сколько попросили, клипы выбираются
//! перемешанными, но всегда одинаково. Каждый клип идёт через весь конвейер приложения с
//! настройками по умолчанию, число спикеров «не знаю», язык речи определяется по клипу.
//! В папку отчёта пишутся `report.md` (он же печатается в конце), `summary.json` (итоги по
//! наборам), `clips/<набор>.jsonl` (каждый клип, худшие раньше) и `audio/<набор>/<номер>.wav`
//! (звук худших клипов). В stderr идут строки `@progress` для хаба.
//! `--engine-threads` задаёт потоки одного движка (по умолчанию 1: на коротких клипах
//! много движков по одному потоку быстрее), `--jobs` число движков, по умолчанию ядра
//! делятся на потоки движка. `--commit` записывает в итоги коммит, на котором шла
//! проверка; `--history <папка>` кладёт копию итогов в историю прогонов; `--exclude <файл>`
//! убирает из проверки клипы с плохим эталоном (`{"<набор>": ["<id>", ...]}`).
//! `--max-speech <с>` задаёт наибольшую длину куска речи, `--drop-music` не распознаёт
//! куски, где вместо речи музыка; без них настройки как в приложении.
//! `--voice-model <имя>` выбирает модель голоса (по умолчанию redimnet2, как в приложении,
//! имена как у eval), `--same-speaker <косинус>` порог «тот же человек» при неизвестном числе
//! спикеров (по умолчанию 0,7 для модели приложения, для остальных 0,6), `--ort-models <папка>`
//! набор ort (по умолчанию `<папка моделей>/ort`). `--min-speaker-sec <с>` наименьшее время
//! речи спикера (по умолчанию как в приложении, 0 выключает правило). `--asr-groups`
//! распознаёт соседние куски с короткой паузой вместе, как приложение на Android.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use nixyon_audio::TARGET_SAMPLE_RATE;
use nixyon_core::{AudioInfo, CancelFlag, EngineInfo, SpeechLanguage};
use nixyon_eval::{
    ClipDer, ClipHypothesis, ClipResult, Dataset, DatasetScore, ExcludedClips, IndexClip,
    KEEP_RUNS, PlanItem, ProgressMeter, QualitySummary, RunInfo, SPEECH_GAP_SEC, SetClip, Truth,
    VoiceModel, WerTotals, archive_run, audio_path, clip_to_wav16k, hypothesis_text, is_commit,
    load_dataset, min_speaker_sec, parse_plan_item, pick_clips, render_quality, run_name,
    speech_spans, wer_totals, worse_first, write_set_clips,
};
use nixyon_metrics::{DER_COLLAR_SEC, SpeakerSpan, diarization_errors};
use nixyon_ort::{OrtManifest, OrtRole};
use nixyon_pipeline::{
    MIN_SPEAKER_SEC, PipelineError, PipelineOptions, SpeakerCount, TranscriptMeta, build_transcript,
};
use nixyon_sherpa::{
    Decoding, ModelManifest, SherpaEngine, SherpaModels, analyze_default, detect_wav_language,
};
use serde::{Serialize, Serializer};

const USAGE: &str = "использование: quality_eval <папка моделей> <корень наборов> --out <папка отчёта> \
     --take <набор>=<минуты|all> [--take ...] [--jobs <число>] [--engine-threads <число>] \
     [--commit <хэш>] [--history <папка>] [--exclude <файл>] [--max-speech <с>] [--drop-music] \
     [--voice-model <имя>] [--same-speaker <косинус>] [--ort-models <папка>] [--min-speaker-sec <с>] \
     [--asr-groups]";
/// Пределы длины куска речи для опытов, секунды.
const MAX_SPEECH_RANGE: std::ops::RangeInclusive<f32> = 2.0..=60.0;

const DEFAULT_ENGINE_THREADS: usize = 1;
/// Модель голоса и порог «тот же человек», как в приложении на Android.
const APP_VOICE_MODEL: VoiceModel = VoiceModel::Redimnet2(OrtRole::Redimnet2);
const APP_SAME_SPEAKER: f64 = 0.7;
// Строка для человека в журнале через каждые 5 % звука; полосу рисует хаб по @progress.
const HUMAN_STEP: f64 = 0.05;

#[derive(Debug, PartialEq)]
struct Args {
    models: PathBuf,
    root: PathBuf,
    out: PathBuf,
    plan: Vec<PlanItem>,
    jobs: usize,
    engine_threads: usize,
    commit: Option<String>,
    history: Option<PathBuf>,
    exclude: Option<PathBuf>,
    max_speech_sec: Option<f32>,
    drop_music: bool,
    voice_model: VoiceModel,
    same_speaker: f64,
    ort_models: Option<PathBuf>,
    min_speaker_sec: f64,
    asr_groups: bool,
}

/// Клип в очереди: из какого набора и сам клип.
struct Work<'a> {
    dataset: usize,
    clip: &'a IndexClip,
}

#[derive(Serialize)]
struct SummaryFile<'a> {
    models_version: &'a str,
    settings: Settings,
    commit: Option<&'a str>,
    /// Когда прогон закончился, секунды Unix.
    finished_at: u64,
    totals: WerTotals,
    workers: usize,
    audio_sec: f64,
    wall_sec: f64,
    datasets: Vec<DatasetFile<'a>>,
}

/// Настройки конвейера в прогоне: по ним сравниваются опыты.
#[derive(Debug, Clone, Copy, Serialize)]
struct Settings {
    max_speech_sec: Option<f32>,
    drop_music: bool,
    #[serde(serialize_with = "voice_name")]
    voice_model: VoiceModel,
    same_speaker: f64,
    min_speaker_sec: f64,
    asr_groups: bool,
}

fn voice_name<S: Serializer>(model: &VoiceModel, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(model.name())
}

impl Settings {
    // Для шапки отчёта: только то, что отличается от приложения.
    fn label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(sec) = self.max_speech_sec {
            parts.push(format!("куски речи до {sec} с"));
        }
        if self.drop_music {
            parts.push("куски с музыкой вместо речи не распознаются".to_owned());
        }
        if self.voice_model != APP_VOICE_MODEL {
            parts.push(format!("модель голоса {}", self.voice_model.name()));
        }
        if self.same_speaker != APP_SAME_SPEAKER {
            parts.push(format!("порог «тот же человек» {}", self.same_speaker));
        }
        if self.min_speaker_sec != MIN_SPEAKER_SEC {
            parts.push(format!("спикер от {} с речи", self.min_speaker_sec));
        }
        if self.asr_groups {
            parts.push("соседние куски распознаются вместе".to_owned());
        }
        parts.join(", ")
    }
}

#[derive(Serialize)]
struct DatasetFile<'a> {
    info: &'a nixyon_eval::DatasetInfo,
    wall_sec: f64,
    excluded: usize,
    summary: &'a QualitySummary,
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1).collect(), cores()) {
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

fn cores() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

fn parse_args(raw: Vec<String>, cores: usize) -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut out = None;
    let mut jobs = None;
    let mut engine_threads = DEFAULT_ENGINE_THREADS;
    let mut commit = None;
    let mut history = None;
    let mut exclude = None;
    let mut max_speech_sec = None;
    let mut drop_music = false;
    let mut voice_model = APP_VOICE_MODEL;
    let mut same_speaker = None;
    let mut ort_models = None;
    let mut min_speaker = MIN_SPEAKER_SEC;
    let mut asr_groups = false;
    let mut plan = Vec::new();
    let mut iter = raw.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(iter.next().ok_or("--out: папка отчёта")?)),
            "--take" => {
                let item =
                    parse_plan_item(&iter.next().ok_or("--take: <набор>=<минуты|all>")?, &plan)?;
                plan.push(item);
            }
            "--jobs" => jobs = Some(positive(iter.next(), "--jobs: число движков, целое от 1")?),
            "--engine-threads" => {
                engine_threads = positive(
                    iter.next(),
                    "--engine-threads: потоки одного движка, целое от 1",
                )?;
            }
            "--commit" => {
                let value = iter.next().ok_or("--commit: хэш коммита")?;
                if !is_commit(&value) {
                    return Err("--commit: 7-40 строчных шестнадцатеричных знаков".into());
                }
                commit = Some(value);
            }
            "--max-speech" => {
                let value = iter
                    .next()
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| MAX_SPEECH_RANGE.contains(v))
                    .ok_or("--max-speech: секунды от 2 до 60")?;
                max_speech_sec = Some(value);
            }
            "--drop-music" => drop_music = true,
            "--voice-model" => {
                voice_model = iter.next().and_then(|v| VoiceModel::parse(&v)).ok_or(
                    "--voice-model: campplus, redimnet2 или redimnet2-b0..b3, b3-vb2, b6-dyn",
                )?;
            }
            "--same-speaker" => {
                let value = iter
                    .next()
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| (-1.0..=1.0).contains(v))
                    .ok_or("--same-speaker: косинус от -1 до 1")?;
                same_speaker = Some(value);
            }
            "--min-speaker-sec" => min_speaker = min_speaker_sec(iter.next())?,
            "--asr-groups" => asr_groups = true,
            "--ort-models" => {
                ort_models = Some(PathBuf::from(
                    iter.next().ok_or("--ort-models: папка набора ort")?,
                ));
            }
            "--exclude" => {
                exclude = Some(PathBuf::from(
                    iter.next().ok_or("--exclude: файл исключённых клипов")?,
                ));
            }
            "--history" => {
                history = Some(PathBuf::from(
                    iter.next().ok_or("--history: папка истории")?,
                ));
            }
            flag if flag.starts_with("--") => return Err(format!("неизвестный флаг {flag}")),
            _ => positional.push(arg),
        }
    }
    let mut positional = positional.into_iter();
    let (Some(models), Some(root), None) =
        (positional.next(), positional.next(), positional.next())
    else {
        return Err("нужны две папки: моделей и наборов".into());
    };
    if plan.is_empty() {
        return Err("нужен хотя бы один --take".into());
    }
    Ok(Args {
        models: PathBuf::from(models),
        root: PathBuf::from(root),
        out: out.ok_or("--out: папка отчёта обязательна")?,
        plan,
        jobs: jobs.unwrap_or_else(|| (cores / engine_threads).max(1)),
        engine_threads,
        commit,
        history,
        exclude,
        max_speech_sec,
        drop_music,
        voice_model,
        // Порог подобран под модель: 0,7 у модели приложения, у остальных прежний общий.
        same_speaker: same_speaker.unwrap_or(if voice_model == APP_VOICE_MODEL {
            APP_SAME_SPEAKER
        } else {
            PipelineOptions::default().same_speaker_cosine
        }),
        ort_models,
        min_speaker_sec: min_speaker,
        asr_groups,
    })
}

fn positive(value: Option<String>, error: &str) -> Result<usize, String> {
    value
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .ok_or_else(|| error.to_string())
}

fn run(args: &Args) -> Result<String, Box<dyn std::error::Error>> {
    let datasets: Vec<Dataset> = args
        .plan
        .iter()
        .map(|item| load_dataset(&args.root, &item.dataset))
        .collect::<Result<_, _>>()?;
    let excluded_clips = match &args.exclude {
        Some(path) => ExcludedClips::load(path)?,
        None => ExcludedClips::default(),
    };
    let mut datasets = datasets;
    let excluded = exclude_clips(&mut datasets, &excluded_clips)?;
    let datasets = datasets;
    let work: Vec<Work> = datasets
        .iter()
        .zip(&args.plan)
        .enumerate()
        .flat_map(|(i, (set, item))| {
            pick_clips(&set.clips, item.minutes.map(|m| m * 60.0))
                .into_iter()
                .map(move |clip| Work { dataset: i, clip })
        })
        .collect();
    let manifest = ModelManifest::load(&args.models)?;
    let ort = if args.voice_model.needs_ort() {
        let dir = args
            .ort_models
            .clone()
            .unwrap_or_else(|| args.models.join("ort"));
        Some(OrtManifest::load(&dir)?)
    } else {
        None
    };
    std::fs::create_dir_all(&args.out)?;
    let jobs = args.jobs.min(work.len()).max(1);
    for (set, item) in datasets.iter().zip(&args.plan) {
        let taken: Vec<&Work> = work
            .iter()
            .filter(|w| datasets[w.dataset].info.id == set.info.id)
            .collect();
        eprintln!(
            "{}: клипов {}, звука {:.1} мин{}",
            set.info.id,
            taken.len(),
            taken.iter().map(|w| w.clip.duration).sum::<f64>() / 60.0,
            if item.minutes.is_none() {
                " (весь набор)"
            } else {
                ""
            }
        );
    }
    eprintln!(
        "движков {jobs} по {} потоку, модели {}, модель голоса {}, порог {}",
        args.engine_threads,
        manifest.version(),
        args.voice_model.name(),
        args.same_speaker
    );
    let started = Instant::now();
    let settings = Settings {
        max_speech_sec: args.max_speech_sec,
        drop_music: args.drop_music,
        voice_model: args.voice_model,
        same_speaker: args.same_speaker,
        min_speaker_sec: args.min_speaker_sec,
        asr_groups: args.asr_groups,
    };
    let results = recognize_all(
        (&manifest, ort.as_ref()),
        &args.root,
        &datasets,
        &work,
        (jobs, args.engine_threads),
        settings,
    )?;
    let wall_sec = started.elapsed().as_secs_f64();
    let scores: Vec<DatasetScore> = datasets
        .iter()
        .enumerate()
        .map(|(i, set)| {
            let mine: Vec<ClipResult> = results
                .iter()
                .filter(|r| r.dataset == set.info.id)
                .cloned()
                .collect();
            let processing: f64 = mine.iter().map(|r| r.processing_sec).sum();
            DatasetScore {
                info: set.info.clone(),
                summary: QualitySummary::of(&mine),
                // Движки работают над всеми наборами сразу, поэтому время набора оценивается
                // по его доле в работе всех движков.
                wall_sec: processing / jobs as f64,
                excluded: excluded[i],
            }
        })
        .collect();
    let run = RunInfo {
        models_version: manifest.version().to_owned(),
        settings: settings.label(),
        workers: jobs,
        wall_sec,
    };
    let report = render_quality(&scores, &run);
    std::fs::write(args.out.join("report.md"), &report)?;
    let finished_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let summary = SummaryFile {
        models_version: manifest.version(),
        settings,
        commit: args.commit.as_deref(),
        finished_at,
        totals: wer_totals(&scores),
        workers: jobs,
        audio_sec: results.iter().map(|r| r.duration_sec).sum(),
        wall_sec,
        datasets: scores
            .iter()
            .map(|s| DatasetFile {
                info: &s.info,
                wall_sec: s.wall_sec,
                excluded: s.excluded,
                summary: &s.summary,
            })
            .collect(),
    };
    std::fs::write(
        args.out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    write_clips(&args.out, &args.root, &datasets, &work, &results)?;
    if let Some(history) = &args.history {
        archive_run(
            &args.out.join("summary.json"),
            history,
            &run_name(finished_at, args.commit.as_deref()),
            KEEP_RUNS,
        )?;
    }
    eprintln!("отчёт: {}", args.out.display());
    Ok(report)
}

// Убирает из наборов клипы с плохим эталоном; возвращает, сколько убрано из каждого.
// Набор, где не осталось клипов, проверять нечем: это ошибка.
fn exclude_clips(datasets: &mut [Dataset], excluded: &ExcludedClips) -> Result<Vec<usize>, String> {
    datasets
        .iter_mut()
        .map(|set| {
            let before = set.clips.len();
            set.clips
                .retain(|clip| !excluded.contains(&set.info.id, &clip.id));
            if set.clips.is_empty() {
                return Err(format!("{}: все клипы исключены", set.info.id));
            }
            Ok(before - set.clips.len())
        })
        .collect()
}

// Файл клипов на каждый набор, худшие раньше, и звук худших клипов.
fn write_clips(
    out: &Path,
    root: &Path,
    datasets: &[Dataset],
    work: &[Work],
    results: &[ClipResult],
) -> std::io::Result<()> {
    for (i, set) in datasets.iter().enumerate() {
        let paths: Vec<(usize, PathBuf)> = work
            .iter()
            .enumerate()
            .filter(|(_, w)| w.dataset == i)
            .map(|(n, w)| (n, audio_path(root, w.clip)))
            .collect();
        let mut clips: Vec<SetClip> = paths
            .iter()
            .map(|(n, path)| SetClip {
                result: &results[*n],
                audio: path,
            })
            .collect();
        clips.sort_by(|a, b| worse_first(a.result, b.result));
        let files = write_set_clips(out, &set.info.id, set.info.has_text, &clips)?;
        if files.audio_failed > 0 {
            eprintln!(
                "{}: звук {} худших клипов не сохранился, они будут без звука",
                set.info.id, files.audio_failed
            );
        }
    }
    Ok(())
}

// Каждый поток держит свой движок и берёт следующий клип из общего счётчика: длинные
// клипы не задерживают остальных. Результаты собираются в порядке очереди.
fn recognize_all(
    manifest: (&ModelManifest, Option<&OrtManifest>),
    root: &Path,
    datasets: &[Dataset],
    work: &[Work],
    (jobs, engine_threads): (usize, usize),
    settings: Settings,
) -> Result<Vec<ClipResult>, String> {
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let (tx, rx) = mpsc::channel::<(usize, ClipResult)>();
    let mut results: Vec<Option<ClipResult>> = vec![None; work.len()];
    let total_sec: f64 = work.iter().map(|w| w.clip.duration).sum();
    let mut meter = ProgressMeter::new(total_sec, work.len());
    let started = Instant::now();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..jobs)
            .map(|worker| {
                let tx = tx.clone();
                let (next, stop) = (&next, &stop);
                scope.spawn(move || -> Result<(), String> {
                    let outcome = worker_loop(
                        manifest,
                        root,
                        datasets,
                        work,
                        (engine_threads, worker),
                        settings,
                        (next, stop),
                        &tx,
                    );
                    if outcome.is_err() {
                        stop.store(true, Ordering::Relaxed);
                    }
                    outcome
                })
            })
            .collect();
        drop(tx);
        let (mut errors, mut words, mut done_sec) = (0, 0, 0.0);
        // Ошибки и слова по каждому набору: общий WER скачет от того, какой набор идёт.
        let mut per_set = vec![(0usize, 0usize); datasets.len()];
        let mut next_human = HUMAN_STEP;
        for (index, result) in rx {
            errors += result.word_errors;
            words += result.words;
            let set = &mut per_set[work[index].dataset];
            set.0 += result.word_errors;
            set.1 += result.words;
            done_sec += result.duration_sec;
            if let Some(line) = meter.clip_done(result.duration_sec, started.elapsed()) {
                eprintln!("{line}");
            }
            let share = if total_sec > 0.0 {
                done_sec / total_sec
            } else {
                1.0
            };
            if share + 1e-9 >= next_human {
                human_progress(share, errors, words, started.elapsed().as_secs_f64());
                let sets: Vec<(&str, usize, usize)> = datasets
                    .iter()
                    .zip(&per_set)
                    .filter(|(set, _)| set.info.has_text)
                    .map(|(set, &(e, w))| (set.info.id.as_str(), e, w))
                    .collect();
                if let Some(line) = sets_wer_line(&sets) {
                    eprintln!("{line}");
                }
                while next_human <= share + 1e-9 {
                    next_human += HUMAN_STEP;
                }
            }
            results[index] = Some(result);
        }
        workers
            .into_iter()
            .map(|w| {
                w.join()
                    .map_err(|_| "поток распознавания упал".to_string())?
            })
            .collect::<Result<Vec<()>, String>>()
    })?;
    results
        .into_iter()
        .enumerate()
        .map(|(i, r)| r.ok_or_else(|| format!("клип {} не обработан", work[i].clip.id)))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn worker_loop(
    (manifest, ort): (&ModelManifest, Option<&OrtManifest>),
    root: &Path,
    datasets: &[Dataset],
    work: &[Work],
    (engine_threads, worker): (usize, usize),
    settings: Settings,
    (next, stop): (&AtomicUsize, &AtomicBool),
    tx: &mpsc::Sender<(usize, ClipResult)>,
) -> Result<(), String> {
    let models = SherpaModels::load_with_threads(
        manifest,
        Decoding::default(),
        i32::try_from(engine_threads).map_err(|_| "слишком много потоков движка")?,
    )
    .map_err(|e| format!("модели не загрузились: {e}"))?;
    // У каждого движка своя сессия модели голоса: движки и так идут параллельно.
    let models = match ort {
        Some(ort) => settings
            .voice_model
            .apply(models, ort, engine_threads, 1)
            .map_err(|e| format!("модель голоса не загрузилась: {e}"))?,
        None => models,
    };
    let engine = SherpaEngine::new(models);
    let options = PipelineOptions {
        max_speech_sec: settings.max_speech_sec,
        drop_music: settings.drop_music,
        same_speaker_cosine: settings.same_speaker,
        min_speaker_sec: settings.min_speaker_sec,
        asr_groups: settings.asr_groups,
        ..PipelineOptions::default()
    };
    let wav = std::env::temp_dir().join(format!(
        "nixyon-quality-{}-{worker}.wav",
        std::process::id()
    ));
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let index = next.fetch_add(1, Ordering::Relaxed);
        let Some(item) = work.get(index) else {
            break;
        };
        let set = &datasets[item.dataset];
        let started = Instant::now();
        let outcome = recognize(
            &engine,
            &options,
            &audio_path(root, item.clip),
            &item.clip.id,
            &wav,
        );
        let der = match (&outcome, &item.clip.truth) {
            (Ok((_, spans)), Some(truth)) => match clip_der(root, truth, spans) {
                Ok(der) => Some(der),
                Err(e) => {
                    eprintln!("{}: DER не посчитан: {e}", item.clip.id);
                    None
                }
            },
            _ => None,
        };
        let outcome = outcome.map(|(hypothesis, _)| hypothesis);
        let result = ClipResult::new(
            &item.clip.id,
            item.clip.text.as_deref().unwrap_or_default(),
            item.clip.duration,
            started.elapsed().as_secs_f64(),
            outcome,
        )
        .from_dataset(&set.info.id, item.clip.speakers)
        .with_der(der);
        if tx.send((index, result)).is_err() {
            break;
        }
    }
    // Временного файла может и не быть, если клипов не досталось.
    let _ = std::fs::remove_file(&wav);
    Ok(())
}

// DER клипа по разметке спикеров `<имя>.truth.json` (путь от корня наборов).
fn clip_der(root: &Path, truth: &str, spans: &[SpeakerSpan]) -> Result<ClipDer, String> {
    let text = std::fs::read_to_string(root.join(truth)).map_err(|e| format!("разметка: {e}"))?;
    let truth = Truth::from_json(&text).map_err(|e| format!("разметка: {e}"))?;
    diarization_errors(&truth.spans(), spans, DER_COLLAR_SEC)
        .map(ClipDer::from)
        .map_err(|e| format!("DER: {e}"))
}

// Текст и число голосов, а также реплики конвейера для DER.
fn recognize(
    engine: &SherpaEngine,
    options: &PipelineOptions,
    audio: &Path,
    id: &str,
    wav: &Path,
) -> Result<(ClipHypothesis, Vec<SpeakerSpan>), String> {
    let converted = clip_to_wav16k(audio, wav)?;
    let cancel = CancelFlag::new();
    // Куски речи от языка не зависят, язык в опциях здесь любой.
    let language = detect_wav_language(
        engine.models(),
        wav,
        &options.engine_options(SpeechLanguage::Ru),
        &cancel,
    )
    .map_err(|e| format!("язык: {e}"))?;
    let (segments, spans) = analyze_default(engine, wav, options, language, &cancel)
        .map_err(|e| format!("движок: {e}"))?;
    match build_transcript(
        segments,
        &spans,
        &[],
        SpeakerCount::Auto,
        options,
        meta(id, converted.duration_sec, language),
        &cancel,
    ) {
        Ok(transcript) => {
            let mut speakers: Vec<u32> = transcript.turns.iter().map(|t| t.speaker).collect();
            speakers.sort_unstable();
            speakers.dedup();
            Ok((
                ClipHypothesis {
                    text: hypothesis_text(&transcript),
                    // Спикеров в клипе единицы, приведение без потерь.
                    speakers: speakers.len() as u32,
                },
                // DER по словам, а не по репликам целиком: паузы внутри реплики не речь.
                speech_spans(&transcript, SPEECH_GAP_SEC),
            ))
        }
        Err(PipelineError::NoSpeech) => Ok((
            ClipHypothesis {
                text: String::new(),
                speakers: 0,
            },
            Vec::new(),
        )),
        Err(e) => Err(format!("конвейер: {e}")),
    }
}

fn meta(id: &str, duration_sec: f64, language: SpeechLanguage) -> TranscriptMeta {
    TranscriptMeta {
        created_at: String::new(),
        audio: AudioInfo {
            file: id.to_owned(),
            duration_sec,
            sample_rate: TARGET_SAMPLE_RATE,
        },
        engine: EngineInfo {
            vad: String::new(),
            asr: String::new(),
            embedding: String::new(),
        },
        language,
    }
}

fn rate_text(errors: usize, words: usize) -> String {
    if words > 0 {
        format!("{:.1}\u{a0}%", errors as f64 * 100.0 / words as f64).replace('.', ",")
    } else {
        "нет".into()
    }
}

// WER по наборам с текстом, где уже есть слова: (набор, ошибок, слов).
fn sets_wer_line(sets: &[(&str, usize, usize)]) -> Option<String> {
    let parts: Vec<String> = sets
        .iter()
        .filter(|(_, _, words)| *words > 0)
        .map(|(id, errors, words)| format!("{id} {}", rate_text(*errors, *words)))
        .collect();
    (!parts.is_empty()).then(|| format!("WER по наборам: {}", parts.join(", ")))
}

fn human_progress(share: f64, errors: usize, words: usize, elapsed: f64) {
    let wer = rate_text(errors, words);
    let left = if share > 0.0 {
        elapsed / share * (1.0 - share)
    } else {
        0.0
    };
    eprintln!(
        "{} готово, WER пока {wer}, прошло {}, осталось около {}",
        format!("{:.1}\u{a0}%", share * 100.0).replace('.', ","),
        clock(elapsed),
        clock(left)
    );
}

// Время для журнала: часы и минуты, а короткое в минутах и секундах.
fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{h} ч {m} мин")
    } else if m > 0 {
        format!("{m} мин {s} с")
    } else {
        format!("{s} с")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &[&str]) -> Result<Args, String> {
        parse_args(raw.iter().map(|s| s.to_string()).collect(), 6)
    }

    #[test]
    fn folders_plan_and_flags_are_parsed_with_defaults() {
        let args = parse(&[
            "m", "d", "--out", "o", "--take", "cv=30", "--take", "talk=all",
        ])
        .unwrap();
        assert_eq!(
            args,
            Args {
                models: "m".into(),
                root: "d".into(),
                out: "o".into(),
                plan: vec![
                    PlanItem {
                        dataset: "cv".into(),
                        minutes: Some(30.0)
                    },
                    PlanItem {
                        dataset: "talk".into(),
                        minutes: None
                    },
                ],
                jobs: 6,
                engine_threads: 1,
                commit: None,
                history: None,
                exclude: None,
                max_speech_sec: None,
                drop_music: false,
                voice_model: APP_VOICE_MODEL,
                same_speaker: 0.7,
                ort_models: None,
                min_speaker_sec: MIN_SPEAKER_SEC,
                asr_groups: false,
            }
        );
        let args = parse(&[
            "m",
            "d",
            "--out",
            "o",
            "--take",
            "cv=1",
            "--commit",
            "91ecf11",
            "--history",
            "h",
        ])
        .unwrap();
        assert_eq!(args.commit.as_deref(), Some("91ecf11"));
        assert_eq!(args.history, Some(PathBuf::from("h")));
        let args = parse(&[
            "m",
            "d",
            "--out",
            "o",
            "--take",
            "cv=1",
            "--exclude",
            "x.json",
        ])
        .unwrap();
        assert_eq!(args.exclude, Some(PathBuf::from("x.json")));
        let tuned = parse(&[
            "m",
            "d",
            "--out",
            "o",
            "--take",
            "cv=1",
            "--max-speech",
            "10",
            "--drop-music",
        ])
        .unwrap();
        assert_eq!((tuned.max_speech_sec, tuned.drop_music), (Some(10.0), true));
        assert!(
            parse(&[
                "m",
                "d",
                "--out",
                "o",
                "--take",
                "cv=1",
                "--max-speech",
                "0.5"
            ])
            .is_err()
        );
        let args = parse(&["m", "d", "--out", "o", "--take", "cv=1", "--jobs", "2"]).unwrap();
        assert_eq!(args.jobs, 2);
    }

    #[test]
    fn voice_model_and_its_threshold_go_together_unless_set() {
        let base = ["m", "d", "--out", "o", "--take", "cv=1"];
        let with = |extra: &[&str]| parse(&[&base[..], extra].concat());
        let cam = with(&["--voice-model", "campplus"]).unwrap();
        assert_eq!(
            (cam.voice_model, cam.same_speaker),
            (VoiceModel::Campplus, 0.6)
        );
        let b2 = with(&["--voice-model", "redimnet2-b2", "--same-speaker", "0.55"]).unwrap();
        assert_eq!(
            (b2.voice_model, b2.same_speaker),
            (VoiceModel::Redimnet2(OrtRole::Redimnet2B2), 0.55)
        );
        let ort = with(&["--ort-models", "x"]).unwrap();
        assert_eq!(ort.ort_models, Some(PathBuf::from("x")));
        assert!(with(&["--voice-model", "titanet"]).is_err());
        assert!(with(&["--voice-model"]).is_err());
        assert!(with(&["--same-speaker", "1.5"]).is_err());
        assert!(with(&["--same-speaker", "NaN"]).is_err());
        assert_eq!(
            with(&["--min-speaker-sec", "3"]).unwrap().min_speaker_sec,
            3.0
        );
        assert!(with(&["--min-speaker-sec", "-1"]).is_err());
        assert!(with(&["--asr-groups"]).unwrap().asr_groups);
    }

    #[test]
    fn settings_name_the_voice_model_and_show_only_what_differs_from_the_app() {
        let app = Settings {
            max_speech_sec: None,
            drop_music: false,
            voice_model: APP_VOICE_MODEL,
            same_speaker: APP_SAME_SPEAKER,
            min_speaker_sec: MIN_SPEAKER_SEC,
            asr_groups: false,
        };
        assert_eq!(app.label(), "");
        let off = Settings {
            min_speaker_sec: 0.0,
            ..app
        };
        assert_eq!(off.label(), "спикер от 0 с речи");
        let grouped = Settings {
            asr_groups: true,
            ..app
        };
        assert_eq!(grouped.label(), "соседние куски распознаются вместе");
        assert_eq!(serde_json::to_value(off).unwrap()["min_speaker_sec"], 0.0);
        let json = serde_json::to_value(app).unwrap();
        assert_eq!(json["voice_model"], "redimnet2");
        assert_eq!(json["same_speaker"], 0.7);
        let cam = Settings {
            voice_model: VoiceModel::Campplus,
            same_speaker: 0.6,
            ..app
        };
        assert_eq!(
            cam.label(),
            "модель голоса campplus, порог «тот же человек» 0.6"
        );
    }

    #[test]
    fn engines_by_default_fill_the_cores_with_their_threads() {
        let four = parse(&[
            "m",
            "d",
            "--out",
            "o",
            "--take",
            "cv=1",
            "--engine-threads",
            "4",
        ])
        .unwrap();
        assert_eq!((four.jobs, four.engine_threads), (1, 4));
        assert!(
            parse(&[
                "m",
                "d",
                "--out",
                "o",
                "--take",
                "cv=1",
                "--engine-threads",
                "0"
            ])
            .is_err()
        );
    }

    #[test]
    fn bad_arguments_are_rejected() {
        assert!(parse(&["m", "d", "--out", "o"]).is_err(), "без --take");
        assert!(parse(&["m", "--out", "o", "--take", "cv=1"]).is_err());
        assert!(parse(&["m", "d", "x", "--out", "o", "--take", "cv=1"]).is_err());
        assert!(parse(&["m", "d", "--take", "cv=1"]).is_err(), "без --out");
        assert!(parse(&["m", "d", "--out", "o", "--take", "cv=1", "--take", "cv=2"]).is_err());
        assert!(parse(&["m", "d", "--out", "o", "--take", "../x=1"]).is_err());
        assert!(parse(&["m", "d", "--out", "o", "--take", "cv=1", "--speed"]).is_err());
        assert!(
            parse(&[
                "m", "d", "--out", "o", "--take", "cv=1", "--commit", "x; rm"
            ])
            .is_err()
        );
    }

    #[test]
    fn wer_by_set_lists_only_sets_that_already_have_words() {
        assert_eq!(
            sets_wer_line(&[("cv", 3, 100), ("radio", 0, 0), ("calls", 1, 8)]).as_deref(),
            Some("WER по наборам: cv 3,0\u{a0}%, calls 12,5\u{a0}%")
        );
        assert_eq!(sets_wer_line(&[("radio", 0, 0)]), None);
    }

    #[test]
    fn excluded_clips_leave_their_set_and_an_emptied_set_is_an_error() {
        let clip = |id: &str| IndexClip {
            id: id.into(),
            audio: format!("{id}.wav"),
            text: Some("раз".into()),
            duration: 1.0,
            speakers: Some(1),
            truth: None,
        };
        let info = |id: &str| nixyon_eval::DatasetInfo {
            id: id.into(),
            title: id.into(),
            kind: nixyon_eval::DatasetKind::Single,
            has_text: true,
            reference: nixyon_eval::ReferenceSource::Human,
            license: "CC0".into(),
            clips: 2,
            hours: 0.1,
        };
        let mut sets = vec![
            Dataset {
                info: info("cv"),
                clips: vec![clip("a"), clip("b")],
            },
            Dataset {
                info: info("talk"),
                clips: vec![clip("a")],
            },
        ];
        let excluded = ExcludedClips::parse(r#"{"cv": ["a"]}"#).unwrap();
        assert_eq!(exclude_clips(&mut sets, &excluded).unwrap(), [1, 0]);
        assert_eq!(sets[0].clips.len(), 1);
        assert_eq!(sets[1].clips.len(), 1, "id «a» другого набора остался");
        let all = ExcludedClips::parse(r#"{"talk": ["a"]}"#).unwrap();
        assert!(exclude_clips(&mut sets, &all).is_err());
    }

    #[test]
    fn log_times_read_in_hours_minutes_and_seconds() {
        assert_eq!(clock(42.0), "42 с");
        assert_eq!(clock(125.0), "2 мин 5 с");
        assert_eq!(clock(5_430.0), "1 ч 30 мин");
    }
}
