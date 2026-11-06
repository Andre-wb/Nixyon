use std::fmt::Write;

use crate::{
    ALL_TEST_SET, ChildAdultRow, ChildAdultScore, L2_GRID, MAX_ADULTS_AS_CHILD, MIN_CHILDREN_FOUND,
    TrainedChildAdult,
};

/// Отчёт классификатора «ребёнок или взрослый» в Markdown: по каждому виду признаков штраф,
/// порог, таблица по наборам и вывод по порогу приёмки на test.
pub fn render_child_adult(results: &[(TrainedChildAdult, Vec<ChildAdultRow>)]) -> String {
    let mut out = String::from("# Ребёнок или взрослый\n");
    let _ = writeln!(
        out,
        "\nПорог приёмки на всех говорящих test: взрослых названо детьми не больше {}, детей \
         найдено не меньше {}. Порог уверенности выбран на train по говорящим из отложенных \
         частей (части по говорящим): наименьший, при котором взрослых названо детьми не \
         больше {}. Точность порога ограничена числом говорящих: при нескольких \
         десятках взрослых допустима одна-две ошибки.",
        percent_of(MAX_ADULTS_AS_CHILD),
        percent_of(MIN_CHILDREN_FOUND),
        percent_of(MAX_ADULTS_AS_CHILD)
    );
    for (trained, rows) in results {
        let model = &trained.model;
        let _ = writeln!(
            out,
            "\n## {}\n\nПризнаков {}, штраф L2 {}, порог уверенности {:.3}{}.\n",
            model.features.key(),
            model.dim(),
            trained.l2,
            model.confidence,
            if trained.confidence_found {
                ""
            } else {
                " (подходящего порога на train нет, ответ всегда «не определено»)"
            }
        );
        let losses: Vec<String> = L2_GRID
            .iter()
            .zip(&trained.l2_losses)
            .map(|(l, loss)| format!("{l}: {loss:.4}"))
            .collect();
        let _ = writeln!(
            out,
            "Потеря на отложенных частях по штрафам: {}.\n",
            losses.join(", ")
        );
        out.push_str(
            "| Набор | По | Детей | Найдено | Названо взрослыми | Взрослых | Найдено | Названо детьми |\n\
             |---|---|---|---|---|---|---|---|\n",
        );
        for row in rows {
            let s = &row.score;
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                row.set,
                if row.speakers {
                    "говорящим"
                } else {
                    "клипам"
                },
                s.children,
                share(s.children_found, s.children),
                share(s.children_as_adult, s.children),
                s.adults,
                share(s.adults_found, s.adults),
                share(s.adults_as_child, s.adults)
            );
        }
        let verdict = rows
            .iter()
            .find(|r| r.set == ALL_TEST_SET)
            .map(|r| verdict_text(&r.score));
        let _ = writeln!(
            out,
            "\nНа всех говорящих test: {}.",
            verdict.unwrap_or("набора test нет")
        );
    }
    out
}

fn verdict_text(s: &ChildAdultScore) -> &'static str {
    if s.passes(MAX_ADULTS_AS_CHILD, MIN_CHILDREN_FOUND) {
        "порог приёмки пройден"
    } else {
        "порог приёмки не пройден"
    }
}

fn share(part: usize, total: usize) -> String {
    if total == 0 {
        "нет".into()
    } else {
        percent_of(part as f64 / total as f64)
    }
}

fn percent_of(share: f64) -> String {
    format!("{:.1} %", 100.0 * share).replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FeatureRole;
    use nixyon_ort::{ChildAdult, ChildAdultFeatures};

    #[test]
    fn the_report_shows_each_feature_kind_and_the_test_verdict() {
        let trained = TrainedChildAdult {
            model: ChildAdult {
                features: ChildAdultFeatures::Redimnet2,
                mean: vec![0.0],
                scale: vec![1.0],
                weights: vec![1.0],
                bias: 0.0,
                confidence: 0.8,
            },
            l2: 0.01,
            l2_losses: vec![0.5; L2_GRID.len()],
            held_out: vec![],
            held_out_speakers: vec![],
            confidence_found: true,
        };
        let row = ChildAdultRow {
            set: ALL_TEST_SET.into(),
            role: FeatureRole::Test,
            speakers: true,
            score: ChildAdultScore::new(&[(true, 0.9), (false, 0.1)], 0.8),
        };
        let text = render_child_adult(&[(trained, vec![row])]);
        assert!(text.contains("## redimnet2"));
        assert!(text.contains(
            "| test, все наборы | говорящим | 1 | 100,0 % | 0,0 % | 1 | 100,0 % | 0,0 % |"
        ));
        assert!(text.contains("порог приёмки пройден"));
    }
}
