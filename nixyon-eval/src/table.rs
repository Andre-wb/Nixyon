use nixyon_metrics::{DerBreakdown, ErrorCount, NamingScore, OverlapScore};

use crate::RecordScore;

/// Таблица Markdown: строка на запись и строка «Всего». В итоге ошибки
/// складываются по записям, доля берётся от суммы. Перекрытия: точность и полнота
/// пометки «говорят одновременно» по времени. Имена: сколько спикеров названо верно,
/// сколько ошибочно и скольких людей с образцом не назвали.
pub fn render_table(rows: &[RecordScore]) -> String {
    let mut out = String::from(
        "| Запись | Спикеры (эталон / найдено) | DER | пропуск | ложная | путаница | WER | CER | cpWER | перекрытия: точность | перекрытия: полнота | имена: верно / ошибочно / пропущено | RTF |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for r in rows {
        out.push_str(&row(
            &r.name,
            &format!("{} / {}", r.truth_speakers, r.found_speakers),
            &Cells {
                der: r.der,
                wer: r.wer,
                cer: r.cer,
                cpwer: r.cpwer,
                overlap: r.overlap,
                naming: r.naming,
                rtf: r.rtf(),
            },
        ));
    }
    let right = rows
        .iter()
        .filter(|r| r.truth_speakers == r.found_speakers)
        .count();
    let audio: f64 = rows.iter().map(|r| r.audio_sec).sum();
    let processing: f64 = rows.iter().map(|r| r.processing_sec).sum();
    out.push_str(&row(
        "Всего",
        &format!("верно {right} из {}", rows.len()),
        &Cells {
            der: rows.iter().map(|r| r.der).sum(),
            wer: sum(rows.iter().map(|r| r.wer)),
            cer: sum(rows.iter().map(|r| r.cer)),
            cpwer: sum(rows.iter().map(|r| r.cpwer)),
            overlap: rows.iter().map(|r| r.overlap).sum(),
            naming: rows.iter().filter_map(|r| r.naming).reduce(|a, b| a + b),
            rtf: (audio > 0.0).then(|| processing / audio),
        },
    ));
    out
}

// Значения одной строки таблицы после имени и спикеров.
struct Cells {
    der: DerBreakdown,
    wer: Option<ErrorCount>,
    cer: Option<ErrorCount>,
    cpwer: Option<ErrorCount>,
    overlap: OverlapScore,
    naming: Option<NamingScore>,
    rtf: Option<f64>,
}

fn row(name: &str, speakers: &str, c: &Cells) -> String {
    let der = &c.der;
    let part = |seconds: f64| {
        percent((der.reference_speech > 0.0).then(|| seconds / der.reference_speech))
    };
    format!(
        "| {name} | {speakers} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
        percent(der.rate()),
        part(der.missed),
        part(der.false_alarm),
        part(der.confusion),
        percent(c.wer.and_then(|e| e.rate())),
        percent(c.cer.and_then(|e| e.rate())),
        percent(c.cpwer.and_then(|e| e.rate())),
        percent(c.overlap.precision()),
        percent(c.overlap.recall()),
        c.naming.map_or_else(dash, |n| format!(
            "{} / {} / {}",
            n.correct,
            n.wrong,
            n.missed()
        )),
        c.rtf.map_or_else(dash, |v| decimal(v, 3)),
    )
}

// Записи без текстового эталона в сумму не входят.
fn sum(counts: impl Iterator<Item = Option<ErrorCount>>) -> Option<ErrorCount> {
    counts.flatten().reduce(|a, b| a + b)
}

pub(crate) fn percent(rate: Option<f64>) -> String {
    rate.map_or_else(dash, |r| format!("{}\u{a0}%", decimal(r * 100.0, 1)))
}

// Русская запись числа: десятичная запятая. Ноль без минуса: сумма пустого списка f64
// равна -0, а крошечное отрицательное число округляется до «-0,0».
pub(crate) fn decimal(value: f64, digits: usize) -> String {
    let text = format!("{value:.digits$}");
    let text = match text.strip_prefix('-') {
        Some(rest) if rest.bytes().all(|b| b == b'0' || b == b'.') => rest.to_owned(),
        _ => text,
    };
    text.replace('.', ",")
}

fn dash() -> String {
    "нет".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(name: &str, wer: Option<ErrorCount>, found: u32) -> RecordScore {
        RecordScore {
            name: name.into(),
            truth_speakers: 2,
            found_speakers: found,
            audio_sec: 10.0,
            processing_sec: 1.0,
            der: DerBreakdown {
                reference_speech: 10.0,
                missed: 1.0,
                false_alarm: 0.0,
                confusion: 0.0,
            },
            wer,
            cer: None,
            cpwer: None,
            overlap: OverlapScore::default(),
            naming: None,
        }
    }

    #[test]
    fn zero_never_prints_with_a_minus() {
        assert_eq!(decimal(-0.0, 1), "0,0");
        assert_eq!(decimal(-0.00001, 2), "0,00");
        assert_eq!(decimal(-1.5, 1), "-1,5");
        assert_eq!(decimal(2.25, 2), "2,25");
    }

    #[test]
    fn total_row_sums_errors_and_skips_records_without_text() {
        let table = render_table(&[
            score(
                "a",
                Some(ErrorCount {
                    errors: 1,
                    reference_len: 10,
                }),
                2,
            ),
            score(
                "b",
                Some(ErrorCount {
                    errors: 3,
                    reference_len: 10,
                }),
                3,
            ),
            score("c", None, 2),
        ]);
        let total = table.lines().last().unwrap();
        assert_eq!(
            total,
            "| Всего | верно 2 из 3 | 10,0\u{a0}% | 10,0\u{a0}% | 0,0\u{a0}% | 0,0\u{a0}% | 20,0\u{a0}% | нет | нет | нет | нет | нет | 0,100 |"
        );
        assert!(
            table.contains(
                "| c | 2 / 2 | 10,0\u{a0}% | 10,0\u{a0}% | 0,0\u{a0}% | 0,0\u{a0}% | нет |"
            )
        );
    }

    #[test]
    fn overlap_columns_show_precision_and_recall_of_the_sum() {
        let mut a = score("a", None, 2);
        a.overlap = OverlapScore {
            reference_sec: 2.0,
            found_sec: 1.0,
            hit_sec: 1.0,
        };
        let mut b = score("b", None, 2);
        b.overlap = OverlapScore {
            reference_sec: 2.0,
            found_sec: 1.0,
            hit_sec: 0.0,
        };
        let table = render_table(&[a, b]);
        assert!(table.contains("| нет | 100,0\u{a0}% | 50,0\u{a0}% | нет | 0,100 |"));
        assert!(table.contains("| нет | 50,0\u{a0}% | 25,0\u{a0}% | нет | 0,100 |"));
    }
}
