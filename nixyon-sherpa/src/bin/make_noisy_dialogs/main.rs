//! Делает зашумлённые копии проверочных диалогов для замера шумоподавления:
//! `make_noisy_dialogs <папка диалогов> <папка DEMAND>`.
//! Для ru2, ru4 и ru6 при ОСШ 20, 10 и 5 дБ пишет `<имя>-snr<дБ>.wav` и копию разметки
//! `<имя>-snr<дБ>.truth.json` в папку диалогов. Шум: настоящие записи помещений из набора
//! DEMAND (Zenodo 1227121), где обычно проходят встречи: офис, столовая, ресторан, первый
//! канал из шестнадцати, по 300 с, поэтому шум не повторяется. Запись переговорной
//! (OMEETING) не берётся: в ней разборчивая чужая речь, а не шум. Речи в выбранных шумах
//! VAD не находит. ОСШ считается по участкам речи из разметки. Состав фиксирован, поэтому
//! каждый запуск даёт те же файлы.

mod noise_mix;

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::process::ExitCode;

use nixyon_audio::{WavReader, WavWriter};

use noise_mix::add_noise;

const RATE: u32 = 16_000;
const SNRS_DB: [u32; 3] = [20, 10, 5];
// Запись и шум к ней.
const RECORDS: [(&str, &str); 3] = [
    ("ru2", "OOFFICE/ch01.wav"),
    ("ru4", "PCAFETER/ch01.wav"),
    ("ru6", "PRESTO/ch01.wav"),
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let [_, dialogs, noise] = args.as_slice() else {
        eprintln!("использование: make_noisy_dialogs <папка диалогов> <папка DEMAND>");
        return ExitCode::from(2);
    };
    match run(Path::new(dialogs), Path::new(noise)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ошибка: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(dialogs: &Path, noise_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for (name, noise_file) in RECORDS {
        let speech = read_mono(&dialogs.join(format!("{name}.wav")))?;
        let noise = read_mono(&noise_dir.join(noise_file))?;
        let truth_text = std::fs::read_to_string(dialogs.join(format!("{name}.truth.json")))?;
        let spans = speech_spans(&truth_text)?;
        for snr in SNRS_DB {
            let mixed = add_noise(&speech, &spans, &noise, f64::from(snr))
                .ok_or_else(|| format!("{name}: нет речи в разметке или шум беззвучный"))?;
            let out = format!("{name}-snr{snr}");
            let mut writer = WavWriter::new(
                BufWriter::new(File::create(dialogs.join(format!("{out}.wav")))?),
                RATE,
            )?;
            writer.write(&mixed)?;
            writer.finish(RATE)?;
            std::fs::write(dialogs.join(format!("{out}.truth.json")), &truth_text)?;
            println!("{out}");
        }
    }
    Ok(())
}

fn read_mono(path: &Path) -> Result<Vec<f32>, String> {
    let named = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = File::open(path).map_err(|e| named(&e))?;
    let mut reader = WavReader::new(BufReader::new(file)).map_err(|e| named(&e))?;
    if reader.format().sample_rate != RATE {
        return Err(named(&format!(
            "нужна частота {RATE} Гц, в файле {}",
            reader.format().sample_rate
        )));
    }
    let mut samples = Vec::new();
    while reader
        .read_mono(usize::try_from(RATE).unwrap_or(16_000), &mut samples)
        .map_err(|e| named(&e))?
        > 0
    {}
    Ok(samples)
}

// Участки речи из разметки в отсчётах: реплики `turns` с `start` и `end` в секундах.
fn speech_spans(truth: &str) -> Result<Vec<(usize, usize)>, String> {
    let value: serde_json::Value = serde_json::from_str(truth).map_err(|e| e.to_string())?;
    let turns = value["turns"]
        .as_array()
        .ok_or("в разметке нет списка turns")?;
    turns
        .iter()
        .map(|t| {
            let at = |key: &str| {
                t[key]
                    .as_f64()
                    .filter(|s| s.is_finite() && *s >= 0.0)
                    // Секунды записи малы, приведение к отсчётам без переполнения.
                    .map(|s| (s * f64::from(RATE)).round() as usize)
                    .ok_or(format!("у реплики нет {key}"))
            };
            Ok((at("start")?, at("end")?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_spans_come_from_truth_turns_in_samples() {
        let truth = r#"{"speakers": 2, "turns": [
            {"speaker": 1, "start": 0.5, "end": 1.0},
            {"speaker": 2, "start": 2.0, "end": 2.25}
        ]}"#;
        assert_eq!(
            speech_spans(truth).unwrap(),
            [(8_000, 16_000), (32_000, 36_000)]
        );
        assert!(speech_spans(r#"{"turns": [{"start": 1.0}]}"#).is_err());
        assert!(speech_spans(r#"{"speakers": 1}"#).is_err());
    }
}
