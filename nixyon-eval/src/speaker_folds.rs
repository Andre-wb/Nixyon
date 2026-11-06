use std::collections::BTreeMap;

/// Номер части для перекрёстной проверки у каждого клипа: все клипы говорящего в одной
/// части, дети и взрослые раскладываются по частям поровну. Раскладка зависит только от
/// имён говорящих, повторный запуск даёт те же части. `clips`: говорящий и класс клипа.
pub fn speaker_folds(clips: &[(&str, bool)], folds: usize) -> Vec<usize> {
    let folds = folds.max(1);
    let mut speakers: BTreeMap<(bool, &str), usize> = BTreeMap::new();
    for (speaker, class) in clips {
        speakers.insert((*class, speaker), 0);
    }
    let mut next = [0usize; 2];
    for ((class, _), fold) in speakers.iter_mut() {
        let n = &mut next[usize::from(*class)];
        *fold = *n % folds;
        *n += 1;
    }
    clips
        .iter()
        .map(|(speaker, class)| speakers[&(*class, *speaker)])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_of_one_speaker_share_a_fold_and_classes_spread_evenly() {
        let clips = [
            ("a", true),
            ("b", true),
            ("a", true),
            ("x", false),
            ("y", false),
        ];
        let f = speaker_folds(&clips, 2);
        assert_eq!(f[0], f[2]);
        assert_ne!(f[0], f[1]);
        assert_ne!(f[3], f[4]);
    }
}
