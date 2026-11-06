use nixyon_cluster::{cosine, min_cost_assignment, unit};
use nixyon_core::VoiceSample;

use crate::VoiceMatch;

/// Центр голоса спикера: номер спикера в расшифровке и единичный вектор.
pub type SpeakerCenter = (u32, Vec<f32>);

/// Человек: имя и единичные эмбеддинги его голоса.
struct Person<'a> {
    name: &'a str,
    voices: Vec<Vec<f32>>,
}

// Эмбеддинги с одним именем принадлежат одному человеку: образец и центры встреч.
fn people(samples: &[VoiceSample]) -> Vec<Person<'_>> {
    let mut people: Vec<Person<'_>> = Vec::new();
    for s in samples {
        let Some(u) = unit(&s.embedding) else {
            continue;
        };
        match people.iter_mut().find(|p| p.name == s.name) {
            Some(p) => p.voices.push(u),
            None => people.push(Person {
                name: &s.name,
                voices: vec![u],
            }),
        }
    }
    people
}

fn person_similarity(center: &[f32], person: &Person<'_>, mode: VoiceMatch) -> f64 {
    match mode {
        VoiceMatch::Max => person
            .voices
            .iter()
            .map(|v| cosine(center, v))
            .fold(f64::NEG_INFINITY, f64::max),
        VoiceMatch::Mean => {
            let dim = center.len();
            let mut sum = vec![0.0_f32; dim];
            for v in &person.voices {
                for (s, x) in sum.iter_mut().zip(v) {
                    *s += x;
                }
            }
            unit(&sum).map_or(f64::NEG_INFINITY, |m| cosine(center, &m))
        }
    }
}

/// Имена спикеров по голосам людей. Эмбеддинги с одним именем считаются одним человеком;
/// с центром спикера человек сравнивается способом `mode`. Спикеры и люди сопоставляются
/// один к одному так, чтобы сумма косинусов была наибольшей; имя ставится, только если
/// косинус пары не ниже `threshold`. Человек, которого нет в записи, остаётся без пары
/// или не проходит порог. Итог: (номер спикера, имя) по возрастанию номера.
pub(crate) fn sample_names(
    centers: &[SpeakerCenter],
    samples: &[VoiceSample],
    threshold: f64,
    mode: VoiceMatch,
) -> Vec<(u32, String)> {
    let voices = people(samples);
    if centers.is_empty() || voices.is_empty() {
        return Vec::new();
    }
    // Квадратная матрица: недостающие строки или столбцы с нулевой ценой ничего
    // не меняют в выборе настоящих пар.
    let n = centers.len().max(voices.len());
    let similarity = |i: usize, j: usize| match (centers.get(i), voices.get(j)) {
        (Some((_, c)), Some(p)) => Some(person_similarity(c, p, mode)),
        _ => None,
    };
    let cost: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| similarity(i, j).map_or(0.0, |s| -s))
                .collect()
        })
        .collect();
    let mut names: Vec<(u32, String)> = min_cost_assignment(&cost)
        .into_iter()
        .enumerate()
        .filter_map(|(i, j)| {
            let s = similarity(i, j)?;
            (s >= threshold).then(|| (centers[i].0, voices[j].name.to_string()))
        })
        .collect();
    names.sort_by_key(|(speaker, _)| *speaker);
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::voice;

    fn sample(name: &str, embedding: Vec<f32>) -> VoiceSample {
        VoiceSample {
            name: name.into(),
            embedding,
        }
    }

    fn center(speaker: u32, id: usize) -> SpeakerCenter {
        (speaker, unit(&voice(id, 0.0)).unwrap())
    }

    #[test]
    fn each_speaker_gets_the_name_of_its_own_voice() {
        let centers = [center(1, 1), center(2, 2)];
        let samples = [
            sample("Борис", voice(2, 0.1)),
            sample("Анна", voice(1, 0.1)),
        ];
        assert_eq!(
            sample_names(&centers, &samples, 0.5, VoiceMatch::Max),
            [(1, "Анна".to_string()), (2, "Борис".to_string())]
        );
    }

    #[test]
    fn sample_below_the_threshold_gives_no_name() {
        let centers = [center(1, 1)];
        // Косинус примерно 0,23: голос другой.
        let samples = [sample("Анна", voice(2, 0.3))];
        assert!(sample_names(&centers, &samples, 0.5, VoiceMatch::Max).is_empty());
        assert_eq!(
            sample_names(&centers, &samples, 0.2, VoiceMatch::Max).len(),
            1
        );
    }

    #[test]
    fn one_sample_names_at_most_one_speaker() {
        // Оба спикера похожи на единственный образец: имя получает самый похожий.
        let centers = [(1, unit(&voice(1, 0.2)).unwrap()), center(2, 1)];
        let samples = [sample("Анна", voice(1, 0.0))];
        assert_eq!(
            sample_names(&centers, &samples, 0.5, VoiceMatch::Max),
            [(2, "Анна".to_string())]
        );
    }

    #[test]
    fn sample_of_someone_absent_names_nobody() {
        let centers = [center(1, 1), center(2, 2)];
        let samples = [
            sample("Анна", voice(1, 0.0)),
            sample("Гость", voice(5, 0.0)),
            sample("Борис", voice(2, 0.0)),
        ];
        let names = sample_names(&centers, &samples, 0.5, VoiceMatch::Max);
        assert_eq!(names, [(1, "Анна".to_string()), (2, "Борис".to_string())]);
    }

    #[test]
    fn no_samples_or_empty_embedding_give_no_names() {
        let centers = [center(1, 1)];
        assert!(sample_names(&centers, &[], 0.5, VoiceMatch::Max).is_empty());
        assert!(sample_names(&centers, &[sample("Анна", vec![])], 0.5, VoiceMatch::Max).is_empty());
        assert!(
            sample_names(&[], &[sample("Анна", voice(1, 0.0))], 0.5, VoiceMatch::Max).is_empty()
        );
    }

    #[test]
    fn several_embeddings_of_one_name_are_one_person() {
        let centers = [center(1, 1), center(2, 2)];
        // У Анны два эмбеддинга, похожих на разных спикеров: имя получает только один.
        let samples = [sample("Анна", voice(1, 0.0)), sample("Анна", voice(2, 0.1))];
        for mode in [VoiceMatch::Max, VoiceMatch::Mean] {
            assert_eq!(sample_names(&centers, &samples, 0.5, mode).len(), 1);
        }
    }

    #[test]
    fn max_takes_the_closest_embedding_and_mean_takes_their_average() {
        let centers = [center(1, 1)];
        // Один эмбеддинг совпадает с центром, другой чужой: наибольший косинус 1,0,
        // со средним около 0,7.
        let samples = [sample("Анна", voice(1, 0.0)), sample("Анна", voice(2, 0.0))];
        assert_eq!(
            sample_names(&centers, &samples, 0.9, VoiceMatch::Max).len(),
            1
        );
        assert!(sample_names(&centers, &samples, 0.9, VoiceMatch::Mean).is_empty());
        assert_eq!(
            sample_names(&centers, &samples, 0.6, VoiceMatch::Mean).len(),
            1
        );
    }
}
