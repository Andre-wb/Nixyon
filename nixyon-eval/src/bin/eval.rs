//! Прогон набора размеченных записей и таблица метрик:
//! `eval <папка моделей> <папка записей> [--speakers auto|truth] [--windows <окно>,<шаг>]
//! [--viterbi <штраф>] [--word-cut] [--no-word-cut] [--no-neural] [--min-speaker-sec <с>] [--asr-groups] [--neural-block <с>] [--samples <файл>] [--sample-threshold <косинус>]
//! [--beam <прибавка>] [--beam-paths <число>] [--greedy] [--hotwords <файл>]
//! [--dump <папка>] [имя записи ...]`.
//! Запись: `<имя>.wav` и разметка `<имя>.truth.json`; без имён берутся все разметки папки.
//! `--speakers truth` даёт конвейеру число спикеров из разметки, по умолчанию «не знаю».
//! `--windows 1.5,0.5` просит у движка эмбеддинги по окнам (длина и шаг в секундах).
//! `--viterbi 0.2` включает пересегментацию Витерби с этим штрафом за смену говорящего;
//! окна для неё движок даёт сам (по умолчанию 1,5 с с шагом 0,5 с или из `--windows`).
//! `--word-cut` режет кусок по словам там, где окна внутри него разных говорящих;
//! `--no-word-cut` выключает разрез (вместе с `--no-neural` каждый кусок VAD размечается целиком, окна не считаются).
//! По умолчанию говорящие берутся из нейросегментации с пометкой перекрытий, как в
//! приложении; `--no-neural` возвращает разметку по кускам VAD с разрезом по словам.
//! `--min-speaker-sec 4` задаёт наименьшее время речи спикера при неизвестном их числе
//! (0 выключает правило, по умолчанию как в приложении). `--asr-groups` распознаёт соседние
//! куски с короткой паузой вместе, как приложение на Android.
//! `--neural-block 60` задаёт длину блока нейросегментации в секундах.
//! `--samples scripts/data/voice-samples.json` даёт конвейеру образцы голоса (клипы ищутся
//! в `<папка моделей>/_download/test-audio`), в таблице появляется оценка имён;
//! `--sample-threshold 0.5` задаёт наименьший косинус для имени.
//! `--library prior6a,prior6b` запоминает голоса по этим записям, как прошлым встречам:
//! центр каждого найденного спикера под именем голоса эталона, с которым он дольше всего
//! совпадает; библиотека идёт конвейеру вместе с образцами. `--voice-match max|mean`:
//! человек с несколькими эмбеддингами сравнивается по наибольшему косинусу или по среднему.
//! `--same-speaker 0.6` задаёт косинус центров, при котором группы при неизвестном числе
//! спикеров сливаются в одного человека.
//! `--sample-threshold`, `--voice-match` и `--same-speaker` принимают и списки через запятую: каждая запись
//! распознаётся один раз, на каждое сочетание выводится своя таблица.
//! По умолчанию распознаватель ищет по лучам, как в приложении; `--beam 1.5` меняет
//! прибавку за горячие слова, `--beam-paths 4` число лучей, `--greedy` включает жадный
//! поиск (для наборов без словаря BPE).
//! `--hotwords <файл>` даёт горячие слова, по одному слову или фразе в строке.
//! `--dump <папка>` сохраняет расшифровку каждой записи в `<папка>/<имя>.json`.
//! `--denoise <gtcrn.onnx>` убирает шум моделью GTCRN после перевода в 16 кГц, до
//! распознавания; время шумоподавления входит во время обработки.
//! Если в наборе есть роль denoiser, как в приложении работает второй VAD по звуку после
//! потокового шумоподавления GTCRN: его куски заменяют куски VAD от 15 с, распознавание и
//! голоса слушают исходную запись. `--no-vad-denoise` выключает его для сравнения.
//! `--voice-model campplus|redimnet2|redimnet2-b0|...` выбирает модель голоса, `--segmenter
//! pyannote|sortformer-int8|sortformer-fp32` нейросегментацию; по умолчанию модели основного
//! набора. Другие модели берутся из набора ort, по умолчанию `<папка моделей>/ort`, другая
//! папка задаётся `--ort-models <папка>`.
//! `--no-truth` прогоняет записи без разметки: нужен только `<имя>.wav`, имена записей и
//! `--dump` обязательны, число спикеров всегда «не знаю». Метрики не считаются, на каждую
//! запись печатается строка: длина звука, время обработки, RTF, число спикеров и крупных
//! спикеров (от 5 с речи).
//! `--combo B=redimnet2,pyannote,0.7` (повторяемый) задаёт сочетания модели голоса,
//! нейросегментации и порога слияния: запись распознаётся один раз, а эмбеддинги и
//! отрезки считаются на каждое сочетание; дамп пишется в `<dump>/<сочетание>/<имя>.json`,
//! строка итога и таблица идут на каждое сочетание. С `--combo` нельзя задавать
//! `--voice-model`, `--segmenter`, `--same-speaker`, `--library` и `--samples`.
//! `--model-threads 4` задаёт потоки на вызов каждой модели (по умолчанию как раньше, до 4),
//! `--embed-jobs 8` число эмбеддингов голоса, которые считаются параллельно, у каждого
//! своя сессия модели голоса (по умолчанию 1, по одному); `--asr-jobs 3` число кусков речи,
//! которые распознаются параллельно, у каждого свой распознаватель (по умолчанию 1). С
//! `--combo` распознавание и нейросегментации сочетаний идут одновременно.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use nixyon_audio::{TARGET_SAMPLE_RATE, convert_to_wav16k};
use nixyon_core::{
    AudioInfo, CancelFlag, EngineInfo, EngineOptions, NEURAL_BLOCK_SEC, SpeechEngine, VoiceSample,
    VoiceSegmenter, WindowSpec, to_json,
};
use nixyon_core::{SegmentResult, VoiceSpan};
use nixyon_eval::{
    Combo, ComboModels, RecordRun, RecordScore, Segmenter, Truth, UnscoredRecord, VoiceModel,
    analyze_combos, library_from_meeting, load_voice_samples, min_speaker_sec, render_table,
    score_record,
};
use nixyon_ort::OrtManifest;
use nixyon_pipeline::{
    PipelineOptions, SpeakerCount, TranscriptMeta, VoiceMatch, build_transcript_with_centers,
};
use nixyon_sherpa::{
    BEAM_PATHS, Decoding, HOTWORD_SCORE, ModelManifest, Parallel, SherpaEngine, SherpaModels,
    denoise_wav, threads,
};

const USAGE: &str = "использование: eval <папка моделей> <папка записей> [--speakers auto|truth] \
     [--windows <окно>,<шаг>] [--viterbi <штраф>] [--word-cut] [--no-word-cut] [--no-neural] [--min-speaker-sec <с>] [--asr-groups] [--neural-block <с>] \
     [--samples <файл>] [--sample-threshold <косинус>] [--same-speaker <косинус>] [--library <запись>,...] [--voice-match max|mean] \
     [--beam <прибавка>] \
     [--beam-paths <число>] [--greedy] [--hotwords <файл>] [--dump <папка>] [--denoise <модель>] [--no-vad-denoise] \
     [--voice-model campplus|redimnet2|redimnet2-b0|redimnet2-b1|redimnet2-b2|redimnet2-b3|redimnet2-b3-vb2|redimnet2-b6-dyn] [--segmenter pyannote|sortformer-int8|sortformer-fp32] \
     [--ort-models <папка>] [--no-truth] [--combo <имя>=<голос>,<сегментация>,<порог> ...] \
     [--model-threads <число>] [--embed-jobs <число>] [--asr-jobs <число>] [имя записи ...]";

const VOICE_MODEL_ERROR: &str = "--voice-model: campplus, redimnet2, redimnet2-b0, redimnet2-b1, \
     redimnet2-b2, redimnet2-b3, redimnet2-b3-vb2 или redimnet2-b6-dyn";

struct Args {
    models: PathBuf,
    records: PathBuf,
    count_from_truth: bool,
    engine: EngineOptions,
    pipeline: PipelineOptions,
    neural_block_sec: f64,
    samples: Option<PathBuf>,
    library: Vec<String>,
    thresholds: Vec<f64>,
    same_speaker: Vec<f64>,
    matches: Vec<VoiceMatch>,
    decoding: Decoding,
    dump: Option<PathBuf>,
    denoise: Option<PathBuf>,
    no_vad_denoise: bool,
    voice_model: VoiceModel,
    segmenter: Segmenter,
    ort_models: Option<PathBuf>,
    no_truth: bool,
    combos: Vec<Combo>,
    model_threads: Option<usize>,
    embed_jobs: usize,
    asr_jobs: usize,
    names: Vec<String>,
}

/// Наибольшее число потоков на модель и параллельных эмбеддингов.
const MAX_THREADS: usize = 64;

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1).collect()) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(table) => {
            print!("{table}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ошибка: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(raw: Vec<String>) -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut count_from_truth = false;
    let mut engine = EngineOptions::default();
    let mut pipeline = PipelineOptions::default();
    let mut neural_block_sec = NEURAL_BLOCK_SEC;
    let mut samples = None;
    let mut library = Vec::new();
    let mut thresholds = Vec::new();
    let mut matches = Vec::new();
    let mut same_speaker = Vec::new();
    let mut decoding = Decoding::default();
    let mut beam_score = HOTWORD_SCORE;
    let mut beam_paths = BEAM_PATHS;
    let mut hotwords_file: Option<PathBuf> = None;
    let mut dump = None;
    let mut denoise = None;
    let mut no_vad_denoise = false;
    let mut voice_model = VoiceModel::default();
    let mut segmenter = Segmenter::default();
    let mut ort_models = None;
    let mut no_truth = false;
    let mut combos: Vec<Combo> = Vec::new();
    let mut model_threads = None;
    let mut embed_jobs = 1;
    let mut asr_jobs = 1;
    let mut picked_models = false;
    let mut iter = raw.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "--speakers" {
            count_from_truth = match iter.next().as_deref() {
                Some("auto") => false,
                Some("truth") => true,
                _ => return Err("--speakers: auto или truth".into()),
            };
        } else if arg == "--windows" {
            let spec = iter.next().as_deref().and_then(parse_windows).ok_or(
                "--windows: длина и шаг окна в секундах через запятую, шаг не длиннее окна",
            )?;
            engine.windows = Some(spec);
            pipeline.window = spec;
        } else if arg == "--viterbi" {
            let penalty = iter
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|p| p.is_finite() && *p >= 0.0)
                .ok_or("--viterbi: штраф за смену говорящего, число не меньше нуля")?;
            pipeline.viterbi_switch_penalty = Some(penalty);
        } else if arg == "--word-cut" {
            pipeline.word_cut = true;
        } else if arg == "--no-word-cut" {
            pipeline.word_cut = false;
        } else if arg == "--no-neural" {
            pipeline.neural_segmentation = false;
        } else if arg == "--asr-groups" {
            pipeline.asr_groups = true;
        } else if arg == "--min-speaker-sec" {
            pipeline.min_speaker_sec = min_speaker_sec(iter.next())?;
        } else if arg == "--neural-block" {
            neural_block_sec = iter
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|b| b.is_finite() && *b > 0.0)
                .ok_or("--neural-block: длина блока в секундах, число больше нуля")?;
        } else if arg == "--beam" {
            let score = iter
                .next()
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|s| s.is_finite() && *s >= 0.0)
                .ok_or("--beam: прибавка за горячие слова, число не меньше нуля")?;
            beam_score = score;
        } else if arg == "--beam-paths" {
            beam_paths = iter
                .next()
                .and_then(|v| v.parse::<i32>().ok())
                .filter(|p| *p >= 1)
                .ok_or("--beam-paths: число лучей, целое от 1")?;
        } else if arg == "--greedy" {
            decoding = Decoding::Greedy;
        } else if arg == "--hotwords" {
            hotwords_file = Some(PathBuf::from(
                iter.next().ok_or("--hotwords: файл горячих слов")?,
            ));
        } else if arg == "--dump" {
            dump = Some(PathBuf::from(iter.next().ok_or("--dump: папка")?));
        } else if arg == "--denoise" {
            denoise = Some(PathBuf::from(
                iter.next().ok_or("--denoise: файл модели GTCRN")?,
            ));
        } else if arg == "--no-vad-denoise" {
            no_vad_denoise = true;
        } else if arg == "--voice-model" {
            picked_models = true;
            voice_model = iter
                .next()
                .as_deref()
                .and_then(VoiceModel::parse)
                .ok_or(VOICE_MODEL_ERROR)?;
        } else if arg == "--segmenter" {
            picked_models = true;
            segmenter = iter
                .next()
                .as_deref()
                .and_then(Segmenter::parse)
                .ok_or("--segmenter: pyannote, sortformer-int8 или sortformer-fp32")?;
        } else if arg == "--ort-models" {
            ort_models = Some(PathBuf::from(
                iter.next().ok_or("--ort-models: папка набора ort")?,
            ));
        } else if arg == "--no-truth" {
            no_truth = true;
        } else if arg == "--combo" {
            let value = iter
                .next()
                .ok_or("--combo: <имя>=<голос>,<сегментация>,<порог>")?;
            combos.push(Combo::parse(&value)?);
        } else if arg == "--model-threads" {
            model_threads = Some(
                count_of(iter.next()).ok_or("--model-threads: потоков на модель, от 1 до 64")?,
            );
        } else if arg == "--embed-jobs" {
            embed_jobs = count_of(iter.next())
                .ok_or("--embed-jobs: параллельных эмбеддингов, от 1 до 64")?;
        } else if arg == "--asr-jobs" {
            asr_jobs = count_of(iter.next())
                .ok_or("--asr-jobs: параллельных распознаваний, от 1 до 64")?;
        } else if arg == "--samples" {
            samples = Some(PathBuf::from(
                iter.next().ok_or("--samples: файл образцов голоса")?,
            ));
        } else if arg == "--library" {
            library = iter
                .next()
                .ok_or("--library: записи через запятую")?
                .split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(String::from)
                .collect();
            if library.is_empty() {
                return Err("--library: записи через запятую".into());
            }
        } else if arg == "--voice-match" {
            let value = iter.next().ok_or("--voice-match: max или mean")?;
            matches = value
                .split(',')
                .map(|m| match m.trim() {
                    "max" => Ok(VoiceMatch::Max),
                    "mean" => Ok(VoiceMatch::Mean),
                    _ => Err("--voice-match: max или mean"),
                })
                .collect::<Result<_, _>>()?;
        } else if arg == "--same-speaker" {
            let value = iter.next().ok_or("--same-speaker: косинус от -1 до 1")?;
            same_speaker = cosines(&value).ok_or("--same-speaker: косинус от -1 до 1")?;
        } else if arg == "--sample-threshold" {
            let value = iter
                .next()
                .ok_or("--sample-threshold: косинус от -1 до 1")?;
            thresholds = value
                .split(',')
                .map(|v| {
                    v.trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|t| t.is_finite() && (-1.0..=1.0).contains(t))
                        .ok_or("--sample-threshold: косинус от -1 до 1")
                })
                .collect::<Result<_, _>>()?;
        } else {
            positional.push(arg);
        }
    }
    let mut positional = positional.into_iter();
    let (Some(models), Some(records)) = (positional.next(), positional.next()) else {
        return Err("не хватает папок".into());
    };
    let hotwords = match hotwords_file {
        Some(file) => std::fs::read_to_string(&file)
            .map_err(|e| format!("--hotwords: {}: {e}", file.display()))?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect(),
        None => Vec::new(),
    };
    if thresholds.is_empty() {
        thresholds.push(pipeline.sample_threshold);
    }
    if matches.is_empty() {
        matches.push(pipeline.voice_match);
    }
    if !combos.is_empty() {
        check_combos(&combos, picked_models || !same_speaker.is_empty())?;
        if !library.is_empty() || samples.is_some() {
            return Err("--combo: --library и --samples не поддерживаются".into());
        }
    }
    if same_speaker.is_empty() {
        same_speaker.push(pipeline.same_speaker_cosine);
    }
    let names: Vec<String> = positional.collect();
    if no_truth {
        check_no_truth(&NoTruthFlags {
            names: &names,
            dump: dump.is_some(),
            count_from_truth,
            library: !library.is_empty(),
            samples: samples.is_some(),
            combos: thresholds.len() * matches.len() * same_speaker.len(),
        })?;
    }
    Ok(Args {
        models: models.into(),
        records: records.into(),
        count_from_truth,
        engine: EngineOptions {
            windows: engine.windows.or(pipeline.engine_options().windows),
            hotwords,
            edges: pipeline.engine_options().edges,
            ..pipeline.engine_options()
        },
        pipeline,
        neural_block_sec,
        samples,
        library,
        thresholds,
        same_speaker,
        matches,
        decoding: match decoding {
            Decoding::Greedy => Decoding::Greedy,
            Decoding::HotwordBeam { .. } => Decoding::HotwordBeam {
                score: beam_score,
                paths: beam_paths,
            },
        },
        dump,
        denoise,
        no_vad_denoise,
        voice_model,
        segmenter,
        ort_models,
        no_truth,
        combos,
        model_threads,
        embed_jobs,
        asr_jobs,
        names,
    })
}

fn count_of(value: Option<String>) -> Option<usize> {
    value?
        .parse::<usize>()
        .ok()
        .filter(|n| (1..=MAX_THREADS).contains(n))
}

// Сочетание само задаёт модель голоса, нейросегментацию и порог, поэтому отдельные флаги
// для них с ним не сочетаются; имена сочетаний становятся папками и не повторяются.
fn check_combos(combos: &[Combo], picked_models: bool) -> Result<(), String> {
    if picked_models {
        return Err("--combo: модель голоса, нейросегментацию и порог задаёт само сочетание, без --voice-model, --segmenter и --same-speaker".into());
    }
    for (i, combo) in combos.iter().enumerate() {
        if combos[..i].iter().any(|c| c.name == combo.name) {
            return Err(format!("--combo: имя {} повторяется", combo.name));
        }
    }
    Ok(())
}

// Что из флагов важно для прогона без разметки.
struct NoTruthFlags<'a> {
    names: &'a [String],
    dump: bool,
    count_from_truth: bool,
    library: bool,
    samples: bool,
    combos: usize,
}

// Без разметки нельзя ни взять из неё число спикеров, ни найти записи по её файлам, ни
// оценить имена. Несколько сочетаний порогов записали бы расшифровки одной записи в один
// файл, поэтому допускается одно.
fn check_no_truth(flags: &NoTruthFlags) -> Result<(), String> {
    if flags.names.is_empty() {
        return Err("--no-truth: нужны имена записей".into());
    }
    if !flags.dump {
        return Err("--no-truth: нужна папка --dump".into());
    }
    if flags.count_from_truth {
        return Err("--no-truth: число спикеров только auto".into());
    }
    if flags.library || flags.samples {
        return Err("--no-truth: для --library и --samples нужна разметка".into());
    }
    if flags.combos > 1 {
        return Err(
            "--no-truth: одно значение --same-speaker, --sample-threshold и --voice-match".into(),
        );
    }
    Ok(())
}

// Список косинусов через запятую, каждый от -1 до 1.
fn cosines(value: &str) -> Option<Vec<f64>> {
    value
        .split(',')
        .map(|v| {
            v.trim()
                .parse::<f64>()
                .ok()
                .filter(|t| t.is_finite() && (-1.0..=1.0).contains(t))
        })
        .collect()
}

fn parse_windows(value: &str) -> Option<WindowSpec> {
    let (window, step) = value.split_once(',')?;
    WindowSpec::new(window.trim().parse().ok()?, step.trim().parse().ok()?)
}

fn run(args: &Args) -> Result<String, Box<dyn std::error::Error>> {
    if args.no_truth {
        return run_unscored(args);
    }
    let names = if args.names.is_empty() {
        all_records(&args.records)?
    } else {
        args.names.clone()
    };
    if names.is_empty() {
        return Err(format!("в {} нет разметок *.truth.json", args.records.display()).into());
    }
    // Все файлы проверяются до загрузки моделей: опечатка в имени видна сразу, а не
    // после минут распознавания предыдущих записей.
    let missing = missing_files(&args.records, args.library.iter().chain(&names), true);
    if !missing.is_empty() {
        let list: Vec<String> = missing.iter().map(|p| p.display().to_string()).collect();
        return Err(format!("нет файлов записей: {}", list.join(", ")).into());
    }
    let (models, extra) = load_models(args)?;
    let engine = SherpaEngine::new(models);
    if let Some(extra) = &extra {
        return run_combos_scored(&engine, extra, args, &names);
    }
    let mut samples = match &args.samples {
        Some(file) => load_voice_samples(
            engine.models(),
            file,
            &args.models.join("_download/test-audio"),
        )?,
        None => Vec::new(),
    };
    for name in &args.library {
        let truth = read_truth(args, name)?;
        let analyzed = analyze(&engine, args, name)?;
        let (transcript, centers) = build_transcript_with_centers(
            analyzed.segments,
            &analyzed.spans,
            &[],
            count(args, &truth),
            &args.pipeline,
            meta(name, analyzed.audio_sec),
            &CancelFlag::new(),
        )?;
        let remembered = library_from_meeting(&transcript, &centers, &truth);
        eprintln!("{name}: в библиотеку {} голосов", remembered.len());
        samples.extend(remembered);
    }
    let mut analyzed = Vec::with_capacity(names.len());
    for name in &names {
        let record = analyze(&engine, args, name)?;
        eprintln!("{name}: распознано за {:.1} с", record.analysis_sec);
        analyzed.push((name, read_truth(args, name)?, record));
    }
    let combos: Vec<(VoiceMatch, f64, f64)> = args
        .matches
        .iter()
        .flat_map(|&m| args.thresholds.iter().map(move |&t| (m, t)))
        .flat_map(|(m, t)| args.same_speaker.iter().map(move |&c| (m, t, c)))
        .collect();
    let mut out = String::new();
    for &(voice_match, threshold, same_speaker) in &combos {
        let options = PipelineOptions {
            voice_match,
            sample_threshold: threshold,
            same_speaker_cosine: same_speaker,
            ..args.pipeline.clone()
        };
        let rows: Vec<RecordScore> = analyzed
            .iter()
            .map(|(name, truth, record)| {
                score_analyzed(
                    args,
                    &options,
                    &samples,
                    name,
                    truth,
                    record,
                    args.dump.as_deref(),
                )
            })
            .collect::<Result<_, _>>()?;
        if combos.len() > 1 {
            out.push_str(&format!(
                "{voice_match:?}, порог {threshold}, один человек от {same_speaker}\n\n"
            ));
        }
        out.push_str(&render_table(&rows));
        if combos.len() > 1 {
            out.push('\n');
        }
    }
    Ok(out)
}

// Записи без разметки: каждая распознаётся, расшифровка пишется в папку дампа, в вывод
// идёт строка с RTF и числом спикеров.
fn run_unscored(args: &Args) -> Result<String, Box<dyn std::error::Error>> {
    let missing = missing_files(&args.records, args.names.iter(), false);
    if !missing.is_empty() {
        let list: Vec<String> = missing.iter().map(|p| p.display().to_string()).collect();
        return Err(format!("нет файлов записей: {}", list.join(", ")).into());
    }
    let dump = args.dump.as_ref().ok_or("--no-truth: нужна папка --dump")?;
    std::fs::create_dir_all(dump)?;
    let (models, extra) = load_models(args)?;
    let engine = SherpaEngine::new(models);
    if let Some(extra) = &extra {
        return run_combos_unscored(&engine, extra, args, dump);
    }
    let options = PipelineOptions {
        voice_match: args.matches[0],
        sample_threshold: args.thresholds[0],
        same_speaker_cosine: args.same_speaker[0],
        ..args.pipeline.clone()
    };
    let mut out = String::new();
    for name in &args.names {
        let record = analyze(&engine, args, name)?;
        eprintln!("{name}: распознано за {:.1} с", record.analysis_sec);
        let started = Instant::now();
        let (transcript, _) = build_transcript_with_centers(
            record.segments,
            &record.spans,
            &[],
            SpeakerCount::Auto,
            &options,
            meta(name, record.audio_sec),
            &CancelFlag::new(),
        )?;
        let processing_sec = record.analysis_sec + started.elapsed().as_secs_f64();
        std::fs::write(dump.join(format!("{name}.json")), to_json(&transcript)?)?;
        let line = UnscoredRecord::new(name, &transcript, record.audio_sec, processing_sec);
        out.push_str(&format!("{line}\n"));
    }
    Ok(out)
}

// Записи с разметкой по сочетаниям: таблица на каждое сочетание.
fn run_combos_scored(
    engine: &SherpaEngine,
    extra: &ComboModels,
    args: &Args,
    names: &[String],
) -> Result<String, Box<dyn std::error::Error>> {
    let mut analyzed = Vec::with_capacity(names.len());
    for name in names {
        let records = analyze_by_combos(engine, extra, args, name)?;
        eprintln!("{name}: распознано по сочетаниям: {}", records.len());
        analyzed.push((name, read_truth(args, name)?, records));
    }
    let mut out = String::new();
    for (i, combo) in args.combos.iter().enumerate() {
        let options = combo_options(args, combo);
        let dump = args.dump.as_ref().map(|d| d.join(&combo.name));
        let rows: Vec<RecordScore> = analyzed
            .iter()
            .map(|(name, truth, records)| {
                score_analyzed(
                    args,
                    &options,
                    &[],
                    name,
                    truth,
                    &records[i],
                    dump.as_deref(),
                )
            })
            .collect::<Result<_, _>>()?;
        out.push_str(&format!("сочетание {}\n\n", combo.name));
        out.push_str(&render_table(&rows));
        out.push('\n');
    }
    Ok(out)
}

// Записи без разметки по сочетаниям: дамп и строка итога на каждое сочетание.
fn run_combos_unscored(
    engine: &SherpaEngine,
    extra: &ComboModels,
    args: &Args,
    dump: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut out = String::new();
    for name in &args.names {
        let records = analyze_by_combos(engine, extra, args, name)?;
        for (combo, record) in args.combos.iter().zip(records) {
            let started = Instant::now();
            let (transcript, _) = build_transcript_with_centers(
                record.segments,
                &record.spans,
                &[],
                SpeakerCount::Auto,
                &combo_options(args, combo),
                meta(name, record.audio_sec),
                &CancelFlag::new(),
            )?;
            let processing_sec = record.analysis_sec + started.elapsed().as_secs_f64();
            let dir = dump.join(&combo.name);
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join(format!("{name}.json")), to_json(&transcript)?)?;
            let label = format!("{name} {}", combo.name);
            let line = UnscoredRecord::new(&label, &transcript, record.audio_sec, processing_sec);
            out.push_str(&format!("{line}\n"));
        }
    }
    Ok(out)
}

// Настройки конвейера сочетания: его порог слияния, остальное по флагам.
fn combo_options(args: &Args, combo: &Combo) -> PipelineOptions {
    PipelineOptions {
        voice_match: args.matches[0],
        sample_threshold: args.thresholds[0],
        same_speaker_cosine: combo.same_speaker,
        ..args.pipeline.clone()
    }
}

// Модели основного набора, в которых модель голоса и нейросегментация заменены по флагам.
// С сочетаниями основной набор остаётся как есть, а модели сочетаний грузятся отдельно.
fn load_models(
    args: &Args,
) -> Result<(SherpaModels, Option<ComboModels>), Box<dyn std::error::Error>> {
    // По умолчанию столько же потоков, сколько sherpa-onnx даёт каждой своей модели.
    let threads = args
        .model_threads
        .unwrap_or_else(|| usize::try_from(threads()).unwrap_or(1));
    let models = SherpaModels::load_parallel(
        &ModelManifest::load(&args.models)?,
        args.decoding,
        i32::try_from(threads).unwrap_or(i32::MAX),
        Parallel {
            embed_jobs: args.embed_jobs,
            asr_jobs: args.asr_jobs,
        },
    )?;
    let models = if args.no_vad_denoise {
        models.without_vad_denoiser()
    } else {
        models
    };
    let dir = args
        .ort_models
        .clone()
        .unwrap_or_else(|| args.models.join("ort"));
    if !args.combos.is_empty() {
        let needs_ort = args
            .combos
            .iter()
            .any(|c| c.voice.needs_ort() || c.segmenter.needs_ort());
        let ort = if needs_ort {
            Some(OrtManifest::load(&dir)?)
        } else {
            None
        };
        let extra = ComboModels::load(&args.combos, ort.as_ref(), threads, args.embed_jobs)?;
        return Ok((models, Some(extra)));
    }
    if !args.voice_model.needs_ort() && !args.segmenter.needs_ort() {
        return Ok((models, None));
    }
    let ort = OrtManifest::load(&dir)?;
    let models = args
        .voice_model
        .apply(models, &ort, threads, args.embed_jobs)?;
    Ok((args.segmenter.apply(models, &ort, threads)?, None))
}

fn all_records(dir: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_string_lossy()
                .strip_suffix(".truth.json")
                .map(str::to_owned)
        })
        .collect();
    names.sort();
    Ok(names)
}

// Звук и разметка каждой записи, которых нет в папке, в порядке имён; без разметки
// проверяется только звук.
fn missing_files<'a>(
    dir: &Path,
    names: impl Iterator<Item = &'a String>,
    with_truth: bool,
) -> Vec<PathBuf> {
    names
        .flat_map(|name| {
            let truth = with_truth.then(|| format!("{name}.truth.json"));
            std::iter::once(format!("{name}.wav")).chain(truth)
        })
        .map(|file| dir.join(file))
        .filter(|path| !path.is_file())
        .collect()
}

fn read_truth(args: &Args, name: &str) -> Result<Truth, Box<dyn std::error::Error>> {
    let path = args.records.join(format!("{name}.truth.json"));
    let text = std::fs::read_to_string(&path).map_err(|e| named(&path, e))?;
    Ok(Truth::from_json(&text).map_err(|e| named(&path, e))?)
}

// Ошибка с путём файла: по одному тексту ошибки ввода-вывода не понять, какой файл виноват.
fn named(path: &Path, e: impl std::fmt::Display) -> String {
    format!("{}: {e}", path.display())
}

fn count(args: &Args, truth: &Truth) -> SpeakerCount {
    if args.count_from_truth {
        SpeakerCount::Known(truth.speakers)
    } else {
        SpeakerCount::Auto
    }
}

struct Analyzed {
    segments: Vec<SegmentResult>,
    spans: Vec<VoiceSpan>,
    audio_sec: f64,
    analysis_sec: f64,
}

fn analyze(
    engine: &SherpaEngine,
    args: &Args,
    name: &str,
) -> Result<Analyzed, Box<dyn std::error::Error>> {
    let ((segments, spans), audio_sec, analysis_sec) = with_wav16k(args, name, |wav| {
        let segments = engine
            .analyze(wav, &args.engine, &CancelFlag::new())
            .map_err(|e| e.to_string())?;
        let spans = if args.pipeline.neural_segmentation {
            let speech: Vec<(f64, f64)> = segments.iter().map(|s| (s.start, s.end)).collect();
            engine
                .voice_spans(wav, &speech, args.neural_block_sec, &CancelFlag::new())
                .map_err(|e| e.to_string())?
        } else {
            Vec::new()
        };
        Ok((segments, spans))
    })?;
    let windows: usize = segments.iter().map(|s| s.windows.len()).sum();
    if windows > 0 {
        eprintln!("{name}: окон с эмбеддингом {windows}");
    }
    if !spans.is_empty() {
        eprintln!("{name}: отрезков нейросегментации {}", spans.len());
    }
    Ok(Analyzed {
        segments,
        spans,
        audio_sec,
        analysis_sec,
    })
}

// Запись распознаётся один раз, каждое сочетание получает свои куски и отрезки. Время
// обработки сочетания: общее (перевод в 16 кГц и распознавание) плюс его собственное.
fn analyze_by_combos(
    engine: &SherpaEngine,
    extra: &ComboModels,
    args: &Args,
    name: &str,
) -> Result<Vec<Analyzed>, Box<dyn std::error::Error>> {
    let neural = args
        .pipeline
        .neural_segmentation
        .then_some(args.neural_block_sec);
    let (results, audio_sec, total_sec) = with_wav16k(args, name, |wav| {
        analyze_combos(
            engine.models(),
            extra,
            &args.combos,
            wav,
            &args.engine,
            neural,
            &CancelFlag::new(),
        )
        .map_err(|e| e.to_string())
    })?;
    let common_sec = total_sec - results.iter().map(|r| r.own_sec).sum::<f64>();
    Ok(results
        .into_iter()
        .map(|r| Analyzed {
            segments: r.segments,
            spans: r.spans,
            audio_sec,
            analysis_sec: common_sec + r.own_sec,
        })
        .collect())
}

// Переводит запись в 16 кГц тем же путём, что приложение (с выравниванием громкости и, по
// флагу, шумоподавлением), и отдаёт её `work`. Возвращает итог, длину звука и время всей
// обработки. Временные файлы удаляются при любом исходе.
fn with_wav16k<T>(
    args: &Args,
    name: &str,
    work: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<(T, f64, f64), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let wav_16k =
        std::env::temp_dir().join(format!("nixyon-eval-{}-{name}.wav", std::process::id()));
    let clean =
        std::env::temp_dir().join(format!("nixyon-eval-{}-{name}-dn.wav", std::process::id()));
    let input = args.records.join(format!("{name}.wav"));
    let converted = convert_to_wav16k(&input, &wav_16k).map_err(|e| named(&input, e));
    let done = converted.as_ref().map_err(String::clone).and_then(|_| {
        let wav = match &args.denoise {
            Some(model) => {
                denoise_wav(model, &wav_16k, &clean).map_err(|e| e.to_string())?;
                &clean
            }
            None => &wav_16k,
        };
        work(wav)
    });
    // Ошибка удаления не важнее ошибки разбора.
    let _ = std::fs::remove_file(&wav_16k);
    let _ = std::fs::remove_file(&clean);
    let done = done?;
    let audio_sec = converted?.duration_sec;
    Ok((done, audio_sec, started.elapsed().as_secs_f64()))
}

// Вторая фаза и счёт по уже распознанной записи; время обработки: распознавание плюс
// вторая фаза.
fn score_analyzed(
    args: &Args,
    options: &PipelineOptions,
    samples: &[VoiceSample],
    name: &str,
    truth: &Truth,
    record: &Analyzed,
    dump: Option<&Path>,
) -> Result<RecordScore, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let audio_sec = record.audio_sec;
    let (transcript, _) = build_transcript_with_centers(
        record.segments.clone(),
        &record.spans,
        samples,
        count(args, truth),
        options,
        meta(name, audio_sec),
        &CancelFlag::new(),
    )?;
    let processing_sec = record.analysis_sec + started.elapsed().as_secs_f64();
    if let Some(dir) = dump {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(format!("{name}.json")), to_json(&transcript)?)?;
    }

    let mut sample_names: Vec<String> = samples.iter().map(|s| s.name.clone()).collect();
    sample_names.sort();
    sample_names.dedup();
    Ok(score_record(&RecordRun {
        name,
        truth,
        transcript: &transcript,
        audio_sec,
        processing_sec,
        samples: (args.samples.is_some() || !args.library.is_empty())
            .then_some(sample_names.as_slice()),
    })?)
}

fn meta(name: &str, duration_sec: f64) -> TranscriptMeta {
    TranscriptMeta {
        created_at: String::new(),
        audio: AudioInfo {
            file: format!("{name}.wav"),
            duration_sec,
            sample_rate: TARGET_SAMPLE_RATE,
        },
        engine: EngineInfo {
            vad: String::new(),
            asr: String::new(),
            embedding: String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(flags: &[&str]) -> Args {
        let raw = ["models", "records"]
            .iter()
            .chain(flags)
            .map(|s| s.to_string())
            .collect();
        parse_args(raw).unwrap()
    }

    #[test]
    fn without_neural_segmentation_word_cut_stays_on_and_asks_for_windows() {
        let args = parse(&["--no-neural"]);
        assert!(args.pipeline.word_cut);
        assert!(args.engine.windows.is_some());
    }

    #[test]
    fn vad_denoiser_from_the_set_is_on_unless_turned_off() {
        assert!(!parse(&[]).no_vad_denoise);
        assert!(parse(&["--no-vad-denoise"]).no_vad_denoise);
    }

    #[test]
    fn denoise_model_is_parsed_and_off_by_default() {
        assert_eq!(parse(&[]).denoise, None);
        assert_eq!(
            parse(&["--denoise", "gtcrn.onnx"]).denoise,
            Some(PathBuf::from("gtcrn.onnx"))
        );
    }

    #[test]
    fn same_speaker_cosines_are_parsed_and_default_to_the_pipeline_value() {
        assert_eq!(
            parse(&[]).same_speaker,
            [PipelineOptions::default().same_speaker_cosine]
        );
        assert_eq!(
            parse(&["--same-speaker", "0.5, 0.7"]).same_speaker,
            [0.5, 0.7]
        );
        assert!(
            parse_args(vec![
                "m".into(),
                "r".into(),
                "--same-speaker".into(),
                "0.5,x".into()
            ])
            .is_err()
        );
    }

    #[test]
    fn library_records_and_voice_match_are_parsed() {
        let args = parse(&["--library", "prior6a, prior6b", "--voice-match", "mean"]);
        assert_eq!(args.library, ["prior6a", "prior6b"]);
        assert_eq!(args.matches, [VoiceMatch::Mean]);
        let sweep = parse(&[
            "--voice-match",
            "max,mean",
            "--sample-threshold",
            "0.4, 0.6",
        ]);
        assert_eq!(sweep.matches, [VoiceMatch::Max, VoiceMatch::Mean]);
        assert_eq!(sweep.thresholds, [0.4, 0.6]);
        let plain = parse(&[]);
        assert_eq!(
            plain.thresholds,
            [PipelineOptions::default().sample_threshold]
        );
        assert!(
            parse_args(vec![
                "m".into(),
                "r".into(),
                "--sample-threshold".into(),
                "0.5,2".into()
            ])
            .is_err()
        );
        assert!(
            parse_args(vec![
                "m".into(),
                "r".into(),
                "--voice-match".into(),
                "avg".into()
            ])
            .is_err()
        );
        assert!(parse_args(vec!["m".into(), "r".into(), "--library".into(), ",".into()]).is_err());
    }

    #[test]
    fn min_speaker_sec_flag_sets_the_pipeline_option() {
        assert_eq!(
            parse(&[]).pipeline.min_speaker_sec,
            PipelineOptions::default().min_speaker_sec
        );
        assert_eq!(
            parse(&["--min-speaker-sec", "0"]).pipeline.min_speaker_sec,
            0.0
        );
        assert!(!parse(&[]).pipeline.asr_groups);
        assert!(parse(&["--asr-groups"]).pipeline.asr_groups);
    }

    #[test]
    fn no_word_cut_without_neural_segmentation_asks_for_no_windows() {
        let args = parse(&["--no-neural", "--no-word-cut"]);
        assert!(!args.pipeline.word_cut);
        assert_eq!(args.pipeline.engine_options().windows, None);
        assert_eq!(args.engine.windows, None);
    }

    #[test]
    fn missing_sound_and_truth_files_are_listed_before_any_work() {
        let dir = std::env::temp_dir().join(format!("nixyon-eval-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ru2.wav"), b"").unwrap();
        std::fs::write(dir.join("ru2.truth.json"), b"").unwrap();
        std::fs::write(dir.join("ru4.wav"), b"").unwrap();
        // Так zsh передаёт список без кавычек: одно имя с пробелами.
        let names = ["ru2".to_string(), "ru4".to_string(), "ru2 ru4".to_string()];
        let missing = missing_files(&dir, names.iter(), true);
        assert_eq!(
            missing,
            [
                dir.join("ru4.truth.json"),
                dir.join("ru2 ru4.wav"),
                dir.join("ru2 ru4.truth.json")
            ]
        );
        assert_eq!(
            missing_files(&dir, names.iter(), false),
            [dir.join("ru2 ru4.wav")]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn parse_err(flags: &[&str]) -> String {
        let raw = ["models", "records"]
            .iter()
            .chain(flags)
            .map(|s| s.to_string())
            .collect();
        parse_args(raw).err().unwrap()
    }

    #[test]
    fn no_truth_needs_names_and_dump_and_is_off_by_default() {
        assert!(!parse(&[]).no_truth);
        let args = parse(&["--no-truth", "--dump", "out", "live"]);
        assert!(args.no_truth);
        assert_eq!(args.names, ["live"]);
        assert!(parse_err(&["--no-truth", "--dump", "out"]).contains("имена"));
        assert!(parse_err(&["--no-truth", "live"]).contains("--dump"));
    }

    #[test]
    fn no_truth_rejects_flags_that_need_truth_or_several_combos() {
        let base = ["--no-truth", "--dump", "out", "live"];
        let with = |extra: &[&str]| {
            let flags: Vec<&str> = base.iter().chain(extra).copied().collect();
            parse_err(&flags)
        };
        assert!(with(&["--speakers", "truth"]).contains("auto"));
        assert!(with(&["--library", "ru2"]).contains("--library"));
        assert!(with(&["--samples", "s.json"]).contains("--samples"));
        assert!(with(&["--same-speaker", "0.6,0.7"]).contains("одно значение"));
        assert!(
            parse(&[
                "--no-truth",
                "--dump",
                "out",
                "--same-speaker",
                "0.7",
                "live"
            ])
            .no_truth
        );
    }

    #[test]
    fn combos_threads_and_embed_jobs_are_parsed_and_default_to_the_old_behaviour() {
        let plain = parse(&[]);
        assert!(plain.combos.is_empty());
        assert_eq!((plain.model_threads, plain.embed_jobs), (None, 1));
        assert_eq!(plain.asr_jobs, 1);
        assert_eq!(parse(&["--asr-jobs", "3"]).asr_jobs, 3);
        let args = parse(&[
            "--combo",
            "A=campplus,pyannote,0.6",
            "--combo",
            "B=redimnet2,pyannote,0.7",
            "--model-threads",
            "2",
            "--embed-jobs",
            "8",
        ]);
        let names: Vec<&str> = args.combos.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["A", "B"]);
        assert_eq!(args.combos[1].same_speaker, 0.7);
        assert_eq!((args.model_threads, args.embed_jobs), (Some(2), 8));
    }

    #[test]
    fn combos_refuse_their_own_settings_given_twice_and_repeated_names() {
        let refused = |flags: &[&str]| {
            let raw = ["m", "r", "--combo", "A=campplus,pyannote,0.6"]
                .iter()
                .chain(flags)
                .map(|s| s.to_string())
                .collect();
            parse_args(raw).err().unwrap()
        };
        for flags in [
            &["--voice-model", "redimnet2"][..],
            &["--segmenter", "sortformer-int8"],
            &["--same-speaker", "0.7"],
        ] {
            assert!(refused(flags).contains("само сочетание"), "{flags:?}");
        }
        assert!(refused(&["--combo", "A=redimnet2,pyannote,0.7"]).contains("повторяется"));
        assert!(refused(&["--library", "ru2"]).contains("--library"));
        assert!(refused(&["--samples", "s.json"]).contains("--samples"));
        for bad in ["0", "65", "x"] {
            for flag in ["--model-threads", "--embed-jobs", "--asr-jobs"] {
                let raw = ["m", "r", flag, bad]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                assert!(parse_args(raw).is_err(), "{flag} {bad}");
            }
        }
    }

    #[test]
    fn unreadable_truth_error_names_the_file() {
        let args = parse(&[]);
        let error = read_truth(&args, "nope").unwrap_err().to_string();
        assert!(error.contains("nope.truth.json"), "{error}");
    }
}
