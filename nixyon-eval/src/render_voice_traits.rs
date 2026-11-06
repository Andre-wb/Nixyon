use std::fmt::Write;

use crate::{BinaryScore, VoiceTraitsReport};

/// Отчёт замера пола и возраста в Markdown: по набору таблица точности и таблица порогов
/// уверенности.
pub fn render_voice_traits(sets: &[(String, VoiceTraitsReport)]) -> String {
    let mut out = String::from("# Пол и возраст по голосу\n");
    for (name, r) in sets {
        let _ = writeln!(
            out,
            "\n## {name}\n\nКлипов {}, говорящих {}, звука {:.1} мин.\n",
            r.clips,
            r.speakers,
            r.audio_sec / 60.0
        );
        out.push_str("| Что | Всего | Верно | Класс a | Класс b |\n|---|---|---|---|---|\n");
        let rows: [(&str, &BinaryScore); 5] = [
            ("Пол, клипы", &r.gender_clips),
            ("Пол, говорящие", &r.gender_speakers),
            ("Пол у детей, клипы", &r.gender_child_clips),
            ("Ребёнок, клипы", &r.child_clips),
            ("Ребёнок, говорящие", &r.child_speakers),
        ];
        for (what, s) in rows {
            if s.total() == 0 {
                continue;
            }
            let _ = writeln!(
                out,
                "| {what} | {} | {} | {} | {} |",
                s.total(),
                percent(s.correct(), s.total()),
                class(&s.a),
                class(&s.b)
            );
        }
        for (what, s) in [
            ("Пол, клипы", &r.gender_clips),
            ("Ребёнок, клипы", &r.child_clips),
        ] {
            if s.total() == 0 {
                continue;
            }
            let _ = writeln!(
                out,
                "\n{what}, порог уверенности:\n\n| Порог | Решено | Ошибок среди решённых |\n|---|---|---|"
            );
            for row in &s.by_confidence {
                let _ = writeln!(
                    out,
                    "| {:.2} | {} | {} |",
                    row.threshold,
                    percent(row.decided, s.total()),
                    percent(row.errors, row.decided)
                );
            }
        }
    }
    out
}

fn class(c: &crate::ClassScore) -> String {
    format!(
        "{} {}/{} ({})",
        c.name,
        c.correct,
        c.total,
        percent(c.correct, c.total)
    )
}

fn percent(part: usize, total: usize) -> String {
    if total == 0 {
        "нет".into()
    } else {
        format!("{:.1} %", 100.0 * part as f64 / total as f64).replace('.', ",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScoredVoiceClip, VoiceGender};
    use nixyon_ort::VoiceTraits;

    #[test]
    fn the_report_names_the_set_and_shows_accuracy_in_percent() {
        let clip = ScoredVoiceClip {
            speaker: "a".into(),
            gender: Some(VoiceGender::Male),
            child: None,
            duration_sec: 60.0,
            traits: VoiceTraits {
                age_years: 30.0,
                female: 0.1,
                male: 0.9,
                child: 0.0,
            },
        };
        let text = render_voice_traits(&[("sps".into(), VoiceTraitsReport::new(&[clip]))]);
        assert!(text.contains("## sps"));
        assert!(text.contains("| Пол, клипы | 1 | 100,0 % |"));
        assert!(!text.contains("Ребёнок, клипы |"));
    }

    #[test]
    fn a_share_of_nothing_is_written_as_none() {
        assert_eq!(percent(0, 0), "нет");
        assert_eq!(percent(1, 3), "33,3 %");
    }
}
