use std::collections::{HashMap, HashSet};

/// Веса клипов для обучения: каждый говорящий весит одинаково, сколько бы клипов у него ни
/// было, и оба класса весят одинаково, сколько бы в них ни было говорящих. Сумма весов 1.
/// `clips`: говорящий и класс клипа.
pub fn balanced_weights(clips: &[(&str, bool)]) -> Vec<f64> {
    let mut per_speaker: HashMap<&str, usize> = HashMap::new();
    for (speaker, _) in clips {
        *per_speaker.entry(speaker).or_default() += 1;
    }
    let mut speakers_in_class = [0usize; 2];
    let mut seen: HashSet<&str> = HashSet::new();
    for (speaker, class) in clips {
        if seen.insert(speaker) {
            speakers_in_class[usize::from(*class)] += 1;
        }
    }
    let classes = speakers_in_class.iter().filter(|n| **n > 0).count() as f64;
    clips
        .iter()
        .map(|(speaker, class)| {
            1.0 / (per_speaker[speaker] as f64
                * speakers_in_class[usize::from(*class)] as f64
                * classes)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speakers_and_classes_weigh_the_same() {
        let w = balanced_weights(&[("a", true), ("a", true), ("b", false), ("c", false)]);
        assert_eq!(w, [0.25, 0.25, 0.25, 0.25]);
        let total: f64 = w.iter().sum();
        assert!((total - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_single_class_still_sums_to_one() {
        let w = balanced_weights(&[("a", true), ("b", true)]);
        assert_eq!(w, [0.5, 0.5]);
    }
}
