//! Собирает проверочные диалоги из живых записей:
//! `make_test_dialogs <папка скачанного> <папка вывода> <файл границ речи> <файл поправок текста>`.
//! Рядом с каждым WAV кладёт разметку `<имя>.truth.json`: кто и когда говорит, а для
//! VoxForge ещё сессию и текст реплики из `etc/PROMPTS` (эталон для WER).
//! Границы реплик в разметке: границы речи внутри клипа без тишины по краям. Для VoxForge
//! они берутся из файла границ (выравнивание по тексту, спорные края проверены на слух):
//! у части клипов шум лишь на 8..14 дБ тише голоса, и правило по громкости там ошибается.
//! Для real3 (чистые записи) границы по громкости. Опечатки PROMPTS правятся по файлу
//! поправок текста; неиспользованная поправка считается ошибкой.
//! real3, ru2, ru4, ru6: клип за клипом с паузами. ru4_fast и ru4_overlap: те же клипы
//! ru4, но смена голоса без паузы или с наложением речи. prior6a и prior6b: «прошлые
//! встречи» тех же шести человек для замера библиотеки голосов, из клипов, которых нет ни
//! в диалогах (клипы 0 и 1 сессии), ни в образцах голоса (2 и 3): клипы 4 и 5 и клипы 6
//! и 7; границы речи по громкости, у этих клипов нет проверенных границ.
//! en2, en4, en6: английская VoxForge (папка `voxforge-en`), три женщины и трое мужчин с
//! родным английским, клип за клипом с паузами; проверенных границ у этих клипов нет,
//! границы по громкости.
//! Состав и порядок реплик фиксированы, поэтому каждый запуск даёт те же файлы.

mod clip;
mod known_bounds;
mod layout;
mod mix;
mod speech_bounds;
mod text_fixes;

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::process::ExitCode;

use nixyon_audio::{WavReader, WavWriter};
use serde_json::json;

use clip::{Clip, DIALOG_CLIPS, Pick, real3_clips, voxforge_clips};
use known_bounds::{KnownBounds, to_samples};
use layout::{ClipShape, Joining, layout};
use mix::mix;
use speech_bounds::speech_bounds;
use text_fixes::TextFixes;

const RATE: u32 = 16_000;
const PAUSES_SEC: [f64; 5] = [0.3, 0.4, 0.6, 0.8, 1.2];
const LEAD_SEC: f64 = 0.5;
const OVERLAPS_SEC: [f64; 3] = [0.5, 1.0, 1.5];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let [_, cache, out, bounds, fixes] = args.as_slice() else {
        eprintln!(
            "использование: make_test_dialogs <папка скачанного> <папка вывода> <файл границ речи> \
             <файл поправок текста>"
        );
        return ExitCode::from(2);
    };
    match run(
        Path::new(cache),
        Path::new(out),
        Path::new(bounds),
        Path::new(fixes),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ошибка: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    cache: &Path,
    out: &Path,
    bounds: &Path,
    fixes: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bounds = KnownBounds::from_json(&std::fs::read_to_string(bounds)?)?;
    let fixes = TextFixes::from_json(&std::fs::read_to_string(fixes)?)?;
    build(
        out,
        "real3",
        &real3_clips(&cache.join("sherpa")),
        Joining::Natural,
    )?;

    let voxforge = cache.join("voxforge");
    let people = ["zend", "gav", "uvgeek", "snoop", "kalan", "polazhenko"];
    for count in [2, 4, 6] {
        let mut clips = Vec::new();
        for person in &people[..count] {
            clips.extend(voxforge_clips(
                &voxforge,
                person,
                DIALOG_CLIPS,
                &bounds,
                &fixes,
            )?);
        }
        shuffle(&mut clips, 7);
        build(out, &format!("ru{count}"), &clips, Joining::Natural)?;
        if count == people.len() {
            check_fixes_used(&fixes, &clips)?;
        }
        if count == 4 {
            build(out, "ru4_fast", &clips, Joining::NoPause)?;
            build(out, "ru4_overlap", &clips, Joining::Overlap(&OVERLAPS_SEC))?;
        }
    }
    let voxforge_en = cache.join("voxforge-en");
    let people_en = [
        "Anniepoo",
        "camdixon",
        "Catbells",
        "tomhannen",
        "Melissa",
        "tsmock",
    ];
    let by_loudness = Pick {
        first: 0,
        known_bounds: false,
    };
    for count in [2, 4, 6] {
        let mut clips = Vec::new();
        for person in &people_en[..count] {
            clips.extend(voxforge_clips(
                &voxforge_en,
                person,
                by_loudness,
                &bounds,
                &fixes,
            )?);
        }
        shuffle(&mut clips, 7);
        build(out, &format!("en{count}"), &clips, Joining::Natural)?;
    }
    for (name, first) in [("prior6a", 4), ("prior6b", 6)] {
        let pick = Pick {
            first,
            known_bounds: false,
        };
        let mut clips = Vec::new();
        for person in &people {
            clips.extend(voxforge_clips(&voxforge, person, pick, &bounds, &fixes)?);
        }
        shuffle(&mut clips, 11);
        build(out, name, &clips, Joining::Natural)?;
    }
    Ok(())
}

fn build(
    out: &Path,
    name: &str,
    clips: &[Clip],
    joining: Joining,
) -> Result<(), Box<dyn std::error::Error>> {
    let audio: Vec<Vec<f32>> = clips
        .iter()
        .map(|c| read_16k_mono(&c.path))
        .collect::<Result<_, _>>()?;
    let mut ids: Vec<&str> = Vec::new();
    let mut shapes = Vec::with_capacity(clips.len());
    for (clip, samples) in clips.iter().zip(&audio) {
        let speaker = ids
            .iter()
            .position(|p| *p == clip.person)
            .unwrap_or_else(|| {
                ids.push(&clip.person);
                ids.len() - 1
            })
            + 1;
        let speech = match clip.speech_sec {
            Some(sec) => to_samples(sec, RATE, samples.len())
                .map_err(|e| format!("{}: {e}", clip.path.display()))?,
            None => speech_bounds(samples)
                .ok_or_else(|| format!("{}: в клипе нет звука", clip.path.display()))?,
        };
        shapes.push(ClipShape {
            speaker,
            len: samples.len(),
            speech,
        });
    }
    let placed = layout(&shapes, joining, RATE, LEAD_SEC, &PAUSES_SEC)?;
    let slices: Vec<&[f32]> = audio.iter().map(Vec::as_slice).collect();
    let samples = mix(&slices, &placed.offsets, placed.total);

    let mut writer = WavWriter::new(
        BufWriter::new(File::create(out.join(format!("{name}.wav")))?),
        RATE,
    )?;
    writer.write(&samples)?;
    writer.finish(RATE)?;

    let seconds = |n: usize| n as f64 / f64::from(RATE);
    let turns: Vec<serde_json::Value> = clips
        .iter()
        .zip(&shapes)
        .zip(&placed.offsets)
        .map(|((clip, shape), &offset)| {
            let mut turn = json!({
                "speaker": shape.speaker,
                "voice": clip.person,
                "start": seconds(offset + shape.speech.0),
                "end": seconds(offset + shape.speech.1),
                "source": clip.path.file_name().map(|n| n.to_string_lossy().into_owned()),
            });
            if let (Some(session), Some(text)) = (&clip.session, &clip.text) {
                turn["session"] = json!(session);
                turn["text"] = json!(text);
            }
            turn
        })
        .collect();
    let truth = json!({ "speakers": ids.len(), "turns": turns });
    std::fs::write(
        out.join(format!("{name}.truth.json")),
        serde_json::to_string_pretty(&truth)?,
    )?;
    println!(
        "{name}: {:.1} с, {} чел., {} реплик",
        seconds(placed.total),
        ids.len(),
        clips.len()
    );
    Ok(())
}

// В самом большом наборе есть все клипы: поправка, которой там нет, устарела.
fn check_fixes_used(fixes: &TextFixes, clips: &[Clip]) -> Result<(), String> {
    for key in fixes.keys() {
        let used = clips.iter().any(|c| {
            let file = c.path.file_name().map(|n| n.to_string_lossy());
            c.session
                .as_deref()
                .zip(file)
                .is_some_and(|(s, f)| format!("{s}/{f}") == key)
        });
        if !used {
            return Err(format!("поправка текста {key} не нашла клипа"));
        }
    }
    Ok(())
}

fn read_16k_mono(path: &Path) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let mut reader = WavReader::new(BufReader::new(File::open(path)?))?;
    let format = reader.format();
    if format.sample_rate != RATE || format.channels != 1 {
        return Err(format!("{}: нужен WAV 16 кГц моно", path.display()).into());
    }
    let mut samples = Vec::new();
    while reader.read_mono(16_384, &mut samples)? > 0 {}
    Ok(samples)
}

// Детерминированное перемешивание Фишера-Йетса на простом линейном генераторе.
fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut state = seed;
    for i in (1..items.len()).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let j = ((state >> 33) % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
}
