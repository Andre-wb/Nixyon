use crate::NamingScore;

/// Отрезок речи с именем: в эталоне имя человека, в гипотезе имя, которое получил
/// спикер (None: имени нет). Секунды от начала записи.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedSpan {
    pub speaker: u32,
    pub name: Option<String>,
    pub start: f64,
    pub end: f64,
}

/// Оценка имён по образцам. Имя спикера гипотезы верно, если это имя человека,
/// с которым у спикера больше всего общего времени. `with_samples`: имена людей,
/// у которых есть образец; из них ждут имени только те, кто есть в эталоне.
pub fn naming_score(
    reference: &[NamedSpan],
    hypothesis: &[NamedSpan],
    with_samples: &[String],
) -> NamingScore {
    let mut people: Vec<&str> = reference.iter().filter_map(|r| r.name.as_deref()).collect();
    people.sort_unstable();
    people.dedup();
    let expected = people
        .iter()
        .filter(|p| with_samples.iter().any(|s| s == *p))
        .count();

    let mut speakers: Vec<(u32, &str)> = hypothesis
        .iter()
        .filter_map(|h| h.name.as_deref().map(|n| (h.speaker, n)))
        .collect();
    speakers.sort_unstable();
    speakers.dedup();

    let mut correct = 0;
    let mut wrong = 0;
    for (speaker, name) in speakers {
        if dominant_person(reference, hypothesis, speaker) == Some(name) {
            correct += 1;
        } else {
            wrong += 1;
        }
    }
    NamingScore {
        expected,
        correct,
        wrong,
    }
}

// Человек эталона, с которым у спикера гипотезы больше всего общего времени.
fn dominant_person<'a>(
    reference: &'a [NamedSpan],
    hypothesis: &[NamedSpan],
    speaker: u32,
) -> Option<&'a str> {
    let mut time: Vec<(&str, f64)> = Vec::new();
    for h in hypothesis.iter().filter(|h| h.speaker == speaker) {
        for r in reference {
            let Some(person) = r.name.as_deref() else {
                continue;
            };
            let overlap = h.end.min(r.end) - h.start.max(r.start);
            if overlap > 0.0 {
                match time.iter_mut().find(|(p, _)| *p == person) {
                    Some((_, t)) => *t += overlap,
                    None => time.push((person, overlap)),
                }
            }
        }
    }
    time.into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(p, _)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(speaker: u32, name: Option<&str>, start: f64, end: f64) -> NamedSpan {
        NamedSpan {
            speaker,
            name: name.map(String::from),
            start,
            end,
        }
    }

    fn reference() -> Vec<NamedSpan> {
        vec![
            span(1, Some("anna"), 0.0, 4.0),
            span(2, Some("boris"), 4.0, 6.0),
            span(1, Some("anna"), 6.0, 9.0),
        ]
    }

    fn samples(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn right_names_are_correct_and_nobody_is_missed() {
        let hyp = [
            span(1, Some("anna"), 0.0, 4.0),
            span(2, Some("boris"), 4.0, 6.0),
            span(1, Some("anna"), 6.0, 9.0),
        ];
        let s = naming_score(&reference(), &hyp, &samples(&["anna", "boris", "gost"]));
        assert_eq!((s.expected, s.correct, s.wrong, s.missed()), (2, 2, 0, 0));
    }

    #[test]
    fn swapped_names_are_wrong_and_both_people_are_missed() {
        let hyp = [
            span(1, Some("boris"), 0.0, 4.0),
            span(2, Some("anna"), 4.0, 6.0),
        ];
        let s = naming_score(&reference(), &hyp, &samples(&["anna", "boris"]));
        assert_eq!((s.correct, s.wrong, s.missed()), (0, 2, 2));
    }

    #[test]
    fn speaker_without_a_name_is_a_miss_not_an_error() {
        let hyp = [span(1, Some("anna"), 0.0, 9.0), span(2, None, 4.0, 6.0)];
        let s = naming_score(&reference(), &hyp, &samples(&["anna", "boris"]));
        assert_eq!((s.correct, s.wrong, s.missed()), (1, 0, 1));
    }

    #[test]
    fn name_of_someone_absent_is_wrong() {
        let hyp = [span(1, Some("gost"), 0.0, 9.0)];
        let s = naming_score(&reference(), &hyp, &samples(&["gost"]));
        assert_eq!((s.expected, s.correct, s.wrong), (0, 0, 1));
    }

    #[test]
    fn person_without_a_sample_is_not_expected() {
        let s = naming_score(&reference(), &[], &samples(&["anna"]));
        assert_eq!((s.expected, s.missed()), (1, 1));
    }

    #[test]
    fn speaker_mixing_two_people_is_judged_by_the_longer_one() {
        // Спикер 1 гипотезы: 4 с Анны и 2 с Бориса.
        let hyp = [span(1, Some("boris"), 0.0, 6.0)];
        let s = naming_score(&reference(), &hyp, &samples(&["anna", "boris"]));
        assert_eq!((s.correct, s.wrong), (0, 1));
    }

    #[test]
    fn scores_add_up() {
        let a = NamingScore {
            expected: 2,
            correct: 1,
            wrong: 1,
        };
        let total: NamingScore = [a, a].into_iter().sum();
        assert_eq!(
            (total.expected, total.correct, total.wrong, total.missed()),
            (4, 2, 2, 2)
        );
    }
}
