use std::fmt::Write;

use crate::table::{decimal, percent};
use crate::{DatasetInfo, DatasetKind, GroupScore, QualitySummary, wer_totals};

/// Что известно о прогоне помимо клипов: для шапки отчёта.
#[derive(Debug, Clone, PartialEq)]
pub struct RunInfo {
    pub models_version: String,
    /// Настройки конвейера, если они не как в приложении; пусто: как в приложении.
    pub settings: String,
    pub workers: usize,
    pub wall_sec: f64,
}

/// Итог одного набора: что за набор и как он прошёл.
#[derive(Debug, Clone, PartialEq)]
pub struct DatasetScore {
    pub info: DatasetInfo,
    pub summary: QualitySummary,
    /// Время, за которое прогон обработал клипы этого набора (все движки сразу).
    pub wall_sec: f64,
    /// Сколько клипов набора исключено из проверки как клипы с плохим эталоном.
    pub excluded: usize,
}

/// Отчёт Markdown: общая таблица по наборам, затем раздел на каждый набор.
pub fn render_quality(scores: &[DatasetScore], run: &RunInfo) -> String {
    let mut out = String::new();
    let audio: f64 = scores.iter().map(|s| s.summary.overall.audio_sec).sum();
    let clips: usize = scores.iter().map(|s| s.summary.overall.clips).sum();
    // Запись в String не падает.
    let _ = writeln!(out, "# Качество распознавания\n");
    let _ = writeln!(out, "Модели: {}", run.models_version);
    if !run.settings.is_empty() {
        let _ = writeln!(out, "Настройки: {}.", run.settings);
    }
    let _ = writeln!(
        out,
        "Наборов {}, клипов {clips}, звука {}, обработка {} на {} движках, {}.\n",
        scores.len(),
        hours(audio),
        hours(run.wall_sec),
        run.workers,
        speed(audio, run.wall_sec)
    );
    out.push_str(&totals(scores));
    let _ = writeln!(
        out,
        "| Набор | Разметка | Эталон | Клипов | Звука | WER | Пустой текст | Голоса | RTF | Скорость |\n|---|---|---|---|---|---|---|---|---|---|"
    );
    for s in scores {
        let o = &s.summary.overall;
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            cell(&s.info.title),
            s.info.kind.label(),
            s.info.reference.label(),
            o.clips,
            hours(o.audio_sec),
            if s.info.has_text {
                percent(o.wer.rate)
            } else {
                "без текста".into()
            },
            share(o.empty, o.clips),
            voices(s),
            rtf(s.summary.rtf),
            speed(o.audio_sec, s.wall_sec)
        );
    }
    out.push('\n');
    for s in scores {
        out.push_str(&render_dataset(s));
    }
    out
}

// WER над таблицей: по наборам, где тексты писал человек, и по всем наборам с текстом.
// Машинный текст сам содержит ошибки, поэтому первая строка ближе к правде о модели.
fn totals(scores: &[DatasetScore]) -> String {
    let totals = wer_totals(scores);
    let mut out = String::new();
    if let Some(human) = totals.human {
        let _ = writeln!(
            out,
            "WER по наборам, где тексты писал человек: {} (наборов {}).\n",
            percent(human.wer.rate),
            human.sets
        );
    }
    if let Some(all) = totals.all {
        let _ = writeln!(
            out,
            "WER по всем наборам с текстом: {} (наборов {}).\n",
            percent(all.wer.rate),
            all.sets
        );
    }
    out
}

// Для одного голоса: доля клипов с лишним голосом; для разговора: доля, где число
// голосов найдено верно.
fn voices(s: &DatasetScore) -> String {
    let summary = &s.summary;
    match s.info.kind {
        DatasetKind::Single => format!(
            "лишний: {}",
            share(summary.multi_speaker, summary.overall.clips)
        ),
        DatasetKind::Dialog if summary.der.is_some() => format!(
            "DER {}, верно: {}",
            percent(summary.der.and_then(|d| d.rate())),
            share(summary.speakers_matched, summary.speakers_known)
        ),
        DatasetKind::Dialog if summary.speakers_known > 0 => {
            format!(
                "верно: {}",
                share(summary.speakers_matched, summary.speakers_known)
            )
        }
        _ => "не проверяется".into(),
    }
}

fn render_dataset(s: &DatasetScore) -> String {
    let mut out = String::new();
    let summary = &s.summary;
    let o = &summary.overall;
    let total = |n: usize| share(n, o.clips);
    let _ = writeln!(out, "## {}\n", cell(&s.info.title));
    let _ = writeln!(
        out,
        "Набор {}, {}, тексты: {}, лицензия {}. Клипов {}, звука {}.\n",
        s.info.id,
        s.info.kind.label(),
        s.info.reference.label(),
        s.info.license,
        o.clips,
        hours(o.audio_sec)
    );
    if s.excluded > 0 {
        let _ = writeln!(out, "Исключено клипов с плохим эталоном: {}.\n", s.excluded);
    }
    let _ = writeln!(out, "| Метрика | Значение |\n|---|---|");
    let mut line = |name: &str, value: String| {
        let _ = writeln!(out, "| {name} | {value} |");
    };
    if s.info.has_text {
        line("WER", percent(o.wer.rate));
        line("CER", percent(o.cer.rate));
        line(
            "WER со знаками и регистром",
            percent(summary.strict_wer.rate),
        );
        line(
            "WER без клипов с цифрами",
            percent(summary.without_digits.wer.rate),
        );
        line(
            "WER, числа приведены к цифрам",
            percent(summary.digits_wer.rate),
        );
        line(
            "WER, дефисы и междометия не считаются",
            percent(summary.lenient_wer.rate),
        );
    }
    line("Пустой текст", total(o.empty));
    line("Текст не получен (ошибка)", total(summary.failures));
    line(
        "Найдено больше одного спикера",
        total(summary.multi_speaker),
    );
    line("Спикеры не найдены (речи нет)", total(summary.no_speakers));
    if summary.speakers_known > 0 {
        line(
            "Число голосов найдено верно",
            share(summary.speakers_matched, summary.speakers_known),
        );
    }
    if let Some(der) = summary.der {
        line("DER", percent(der.rate()));
        line("пропуск речи", percent(der.share(der.missed)));
        line("ложная речь", percent(der.share(der.false_alarm)));
        line("путаница спикеров", percent(der.share(der.confusion)));
    }
    line("RTF одного движка", rtf(summary.rtf));
    out.push('\n');
    if !s.info.has_text {
        return out;
    }
    out.push_str(&groups("По длине клипа", &summary.by_duration));
    out.push_str(&groups(
        "По цифрам в ответе (эталон пишет числа словами)",
        &[summary.with_digits.clone(), summary.without_digits.clone()],
    ));
    if !summary.failure_reasons.is_empty() {
        let _ = writeln!(
            out,
            "### Почему текст не получен\n\n| Причина | Клипов |\n|---|---|"
        );
        for (reason, count) in &summary.failure_reasons {
            let _ = writeln!(out, "| {} | {count} |", cell(reason));
        }
        out.push('\n');
    }
    if !summary.substitutions.is_empty() {
        let _ = writeln!(
            out,
            "### Частые замены слов\n\n| Эталон | Распознано | Раз |\n|---|---|---|"
        );
        for sub in &summary.substitutions {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                cell(&sub.reference),
                cell(&sub.hypothesis),
                sub.count
            );
        }
        out.push('\n');
    }
    let _ = writeln!(
        out,
        "### Худшие клипы ({})\n\nОшибки и WER без ошибок записи: числа, единицы, дефисы, междометия и разговорные формы не считаются.\n\n| Клип | Ошибок | WER | Эталон | Распознано |\n|---|---|---|---|---|",
        summary.worst.len()
    );
    for c in &summary.worst {
        let _ = writeln!(
            out,
            "| {} | {} из {} | {} | {} | {} |",
            c.id,
            c.relaxed_word_errors,
            c.relaxed_words,
            percent(c.relaxed_wer()),
            cell(&c.reference),
            cell(&c.hypothesis)
        );
    }
    out.push('\n');
    out
}

fn groups(title: &str, groups: &[GroupScore]) -> String {
    let mut out = format!(
        "### {title}\n\n| Группа | Клипов | Звука | WER | CER | Пустой текст |\n|---|---|---|---|---|---|\n"
    );
    for g in groups {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} |",
            g.label,
            g.clips,
            hours(g.audio_sec),
            percent(g.wer.rate),
            percent(g.cer.rate),
            share(g.empty, g.clips)
        );
    }
    out.push('\n');
    out
}

fn rtf(value: Option<f64>) -> String {
    value.map_or_else(|| "нет".into(), |r| decimal(r, 3))
}

fn speed(audio_sec: f64, wall_sec: f64) -> String {
    if wall_sec > 0.0 {
        format!("{} × реального времени", decimal(audio_sec / wall_sec, 1))
    } else {
        "нет".into()
    }
}

fn share(part: usize, whole: usize) -> String {
    let rate = (whole > 0).then(|| part as f64 / whole as f64);
    format!("{part} ({})", percent(rate))
}

fn hours(seconds: f64) -> String {
    if seconds >= 3600.0 {
        format!("{}\u{a0}ч", decimal(seconds / 3600.0, 2))
    } else if seconds >= 60.0 {
        format!("{}\u{a0}мин", decimal(seconds / 60.0, 1))
    } else {
        format!("{}\u{a0}с", decimal(seconds, 1))
    }
}

// Текст клипа внутри ячейки таблицы: черта ломает строку таблицы, перевод строки тоже.
fn cell(text: &str) -> String {
    let text = text.replace('|', "\\|").replace(['\n', '\r'], " ");
    if text.trim().is_empty() {
        "(пусто)".into()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipHypothesis, ClipResult, ReferenceSource};

    fn run() -> RunInfo {
        RunInfo {
            models_version: "v1".into(),
            settings: String::new(),
            workers: 3,
            wall_sec: 10.0,
        }
    }

    fn info(id: &str, kind: DatasetKind, has_text: bool) -> DatasetInfo {
        DatasetInfo {
            id: id.into(),
            title: format!("Набор {id}"),
            kind,
            has_text,
            reference: ReferenceSource::Human,
            license: "CC0".into(),
            clips: 2,
            hours: 0.1,
        }
    }

    fn heard(id: &str, reference: &str, text: &str, duration: f64, speakers: u32) -> ClipResult {
        ClipResult::new(
            id,
            reference,
            duration,
            duration / 10.0,
            Ok(ClipHypothesis {
                text: text.into(),
                speakers,
            }),
        )
    }

    #[test]
    fn report_shows_totals_groups_and_worst_clips() {
        let results = [
            heard("a", "раз | два", "раз три", 4.0, 2),
            ClipResult::new("b", "раз", 6.0, 0.6, Err("нет речи".into())),
        ];
        let score = DatasetScore {
            info: info("cv", DatasetKind::Single, true),
            summary: QualitySummary::of(&results),
            wall_sec: 10.0,
            excluded: 3,
        };
        let text = render_quality(&[score], &run());
        assert!(text.contains("| WER | 66,7\u{a0}% |"), "{text}");
        assert!(
            text.contains("| WER, числа приведены к цифрам | 66,7\u{a0}% |"),
            "{text}"
        );
        assert!(
            text.contains("| WER, дефисы и междометия не считаются | 66,7\u{a0}% |"),
            "{text}"
        );
        assert!(
            text.contains("| Найдено больше одного спикера | 1 (50,0\u{a0}%) |"),
            "{text}"
        );
        assert!(
            text.contains("| Набор cv | один голос | человек | 2 | 10,0\u{a0}с | 66,7\u{a0}% | 1 (50,0\u{a0}%) | лишний: 1 (50,0\u{a0}%) | 0,100 | 1,0 × реального времени |"),
            "{text}"
        );
        assert!(text.contains("| нет речи | 1 |"), "{text}");
        assert!(
            text.contains("Исключено клипов с плохим эталоном: 3."),
            "{text}"
        );
        assert!(text.contains("раз \\| два"), "{text}");
        assert!(
            text.contains("| b | 1 из 1 | 100,0\u{a0}% | раз | (пусто) |"),
            "{text}"
        );
        assert!(text.contains("| от 5 до 8 с | 1 |"), "{text}");
        assert!(text.contains("| от 8 с | 0 | 0,0\u{a0}с |"), "{text}");
        // Замены считаются после приведения чисел к цифрам: «два» и «три» как 2 и 3.
        assert!(text.contains("| 2 | 3 | 1 |"), "{text}");
        assert!(
            text.contains("| Спикеры не найдены (речи нет) | 0 (0,0\u{a0}%) |"),
            "{text}"
        );
    }

    #[test]
    fn wer_over_human_texts_is_shown_apart_from_wer_over_all_texts() {
        let human = DatasetScore {
            info: info("calls", DatasetKind::Single, true),
            summary: QualitySummary::of(&[heard("a", "раз два", "раз три", 2.0, 1)]),
            wall_sec: 1.0,
            excluded: 0,
        };
        let mut auto_info = info("radio", DatasetKind::Single, true);
        auto_info.reference = ReferenceSource::Auto;
        let auto = DatasetScore {
            info: auto_info,
            summary: QualitySummary::of(&[heard("a", "раз два", "три четыре", 2.0, 1)]),
            wall_sec: 1.0,
            excluded: 0,
        };
        let mut silent_info = info("voice", DatasetKind::Single, false);
        silent_info.reference = ReferenceSource::Human;
        let silent = DatasetScore {
            info: silent_info,
            summary: QualitySummary::of(&[heard("a", "", "раз", 2.0, 1)]),
            wall_sec: 1.0,
            excluded: 0,
        };
        let text = render_quality(&[human, auto, silent], &run());
        assert!(
            text.contains("WER по наборам, где тексты писал человек: 50,0\u{a0}% (наборов 1)."),
            "{text}"
        );
        assert!(
            text.contains("WER по всем наборам с текстом: 75,0\u{a0}% (наборов 2)."),
            "{text}"
        );
        assert!(
            text.contains("| Набор radio | один голос | машина |"),
            "{text}"
        );
    }

    #[test]
    fn a_set_without_text_shows_no_word_errors_and_a_dialog_shows_found_voices() {
        let silent = DatasetScore {
            info: info("voice", DatasetKind::Single, false),
            summary: QualitySummary::of(&[heard("a", "", "раз", 2.0, 1)]),
            wall_sec: 1.0,
            excluded: 0,
        };
        let calls = DatasetScore {
            info: info("calls", DatasetKind::Dialog, true),
            summary: QualitySummary::of(&[
                heard("a", "раз", "раз", 60.0, 2).from_dataset("calls", Some(2)),
                heard("b", "раз", "раз", 60.0, 3).from_dataset("calls", Some(2)),
            ]),
            wall_sec: 6.0,
            excluded: 0,
        };
        let text = render_quality(&[silent, calls], &run());
        assert!(text.contains("| без текста |"), "{text}");
        let silent_part =
            &text[text.find("## Набор voice").unwrap()..text.find("## Набор calls").unwrap()];
        assert!(!silent_part.contains("| WER |"), "{silent_part}");
        assert!(!silent_part.contains("Худшие клипы"), "{silent_part}");
        assert!(text.contains("верно: 1 (50,0\u{a0}%)"), "{text}");
        assert!(!text.contains("| DER |"), "без разметки DER не считается");
        assert!(
            text.contains("| Число голосов найдено верно | 1 (50,0\u{a0}%) |"),
            "{text}"
        );
    }

    #[test]
    fn a_dialog_with_speaker_truth_shows_der_and_its_parts() {
        let der = crate::ClipDer {
            reference_speech: 20.0,
            missed: 1.0,
            false_alarm: 0.5,
            confusion: 0.5,
        };
        let talk = DatasetScore {
            info: info("talk", DatasetKind::Dialog, true),
            summary: QualitySummary::of(&[heard("a", "раз", "раз", 30.0, 2)
                .from_dataset("talk", Some(2))
                .with_der(Some(der))]),
            wall_sec: 1.0,
            excluded: 0,
        };
        let text = render_quality(&[talk], &run());
        assert!(text.contains("| DER | 10,0\u{a0}% |"), "{text}");
        assert!(text.contains("| пропуск речи | 5,0\u{a0}% |"), "{text}");
        assert!(
            text.contains("| путаница спикеров | 2,5\u{a0}% |"),
            "{text}"
        );
        assert!(
            text.contains("DER 10,0\u{a0}%, верно: 1 (100,0\u{a0}%)"),
            "{text}"
        );
    }

    #[test]
    fn durations_read_in_seconds_minutes_or_hours() {
        assert_eq!(hours(30.0), "30,0\u{a0}с");
        assert_eq!(hours(90.0), "1,5\u{a0}мин");
        assert_eq!(hours(118_746.5), "32,99\u{a0}ч");
    }
}
