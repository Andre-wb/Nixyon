use crate::{ahc, centroids, cosine, nearest};

/// AHC на `k` говорящих, где говорящий: кластер, в котором речи не меньше `min_sec`
/// секунд. Шум и обрывки речи далеки от всех голосов и при простом AHC занимают
/// кластеры, а два настоящих похожих голоса тогда сливаются. Поэтому кластеров берётся
/// больше, пока крупных не станет `k`; точки мелких уходят к ближайшему крупному. Крупные
/// кластеры, чьи центры похожи на `same_speaker_cosine` и больше, затем сливаются: лишний
/// кластер мог разрезать одного человека.
///
/// `durations`: секунды речи каждой точки. `min_sec` не больше нуля: обычный AHC.
/// Номера кластеров с 0 в порядке первого появления.
pub fn sized_ahc(
    units: &[Vec<f32>],
    durations: &[f64],
    k: usize,
    min_sec: f64,
    same_speaker_cosine: f64,
) -> Vec<usize> {
    if min_sec <= 0.0 || units.is_empty() {
        return ahc(units, k);
    }
    let k = k.clamp(1, units.len());
    let mut tried = k;
    loop {
        let labels = ahc(units, tried);
        let large: Vec<usize> = (0..tried)
            .filter(|&c| speech_of(&labels, durations, c) >= min_sec)
            .collect();
        if large.len() >= k || tried >= units.len() {
            // Ни одного крупного: запись слишком коротка для правила, обычный AHC.
            if large.is_empty() {
                return ahc(units, k);
            }
            let joined = join_small(units, &labels, tried, &large);
            return merge_similar(units, joined, large.len(), same_speaker_cosine);
        }
        tried += 1;
    }
}

fn speech_of(labels: &[usize], durations: &[f64], cluster: usize) -> f64 {
    labels
        .iter()
        .zip(durations)
        .filter(|&(&l, _)| l == cluster)
        .map(|(_, &d)| d)
        .sum()
}

// Точки мелких кластеров уходят к ближайшему центру крупного; номера: 0..large.len().
fn join_small(units: &[Vec<f32>], labels: &[usize], k: usize, large: &[usize]) -> Vec<usize> {
    let all = centroids(units, labels, k);
    let large_centers: Vec<Option<Vec<f32>>> = large.iter().map(|&c| all[c].clone()).collect();
    labels
        .iter()
        .zip(units)
        .map(|(&l, u)| {
            large
                .iter()
                .position(|&c| c == l)
                .or_else(|| nearest(&large_centers, u))
                .unwrap_or(0)
        })
        .collect()
}

// Сливает два самых похожих кластера, пока косинус их центров не меньше порога.
fn merge_similar(
    units: &[Vec<f32>],
    mut labels: Vec<usize>,
    mut k: usize,
    same_speaker_cosine: f64,
) -> Vec<usize> {
    while k > 1 {
        let centers = centroids(units, &labels, k);
        let mut best: Option<(f64, usize, usize)> = None;
        for i in 0..k {
            for j in (i + 1)..k {
                if let (Some(a), Some(b)) = (&centers[i], &centers[j]) {
                    let c = cosine(a, b);
                    if best.is_none_or(|(top, _, _)| c > top) {
                        best = Some((c, i, j));
                    }
                }
            }
        }
        let Some((top, keep, gone)) = best else { break };
        if top < same_speaker_cosine {
            break;
        }
        for l in &mut labels {
            if *l == gone {
                *l = keep;
            } else if *l > gone {
                *l -= 1;
            }
        }
        k -= 1;
    }
    first_appearance(labels)
}

fn first_appearance(labels: Vec<usize>) -> Vec<usize> {
    let mut order: Vec<usize> = Vec::new();
    labels
        .into_iter()
        .map(|l| {
            order.iter().position(|&o| o == l).unwrap_or_else(|| {
                order.push(l);
                order.len() - 1
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit;

    fn near(x: f32, y: f32, z: f32, w: f32, wobble: f32) -> Vec<f32> {
        unit(&[x + wobble, y, z - wobble, w]).unwrap()
    }

    // Голоса A и B похожи (косинус около 0,6), но это разные люди; шум далёк от обоих.
    fn two_voices_and_noise() -> (Vec<Vec<f32>>, Vec<f64>) {
        let mut units = Vec::new();
        for w in [0.0, 0.02, -0.02] {
            units.push(near(1.0, 0.0, 0.0, 0.0, w));
        }
        for w in [0.0, 0.02, -0.02] {
            units.push(near(0.6, 0.8, 0.0, 0.0, w));
        }
        units.push(near(0.0, 0.0, 0.0, 1.0, 0.0));
        (units, vec![2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 1.5])
    }

    #[test]
    fn noise_does_not_take_a_speaker_and_two_similar_voices_stay_apart() {
        let (units, durations) = two_voices_and_noise();
        // Простой AHC на двоих отдаёт шуму целый кластер и сливает A и B.
        let plain = ahc(&units, 2);
        assert_eq!(plain[0], plain[3]);
        let labels = sized_ahc(&units, &durations, 2, 4.0, 0.7);
        assert_eq!(&labels[..6], [0, 0, 0, 1, 1, 1]);
        assert!(labels[6] < 2, "шум ушёл к ближайшему голосу: {labels:?}");
    }

    #[test]
    fn halves_of_one_voice_are_merged_back() {
        let mut units = Vec::new();
        for w in [0.0, 0.02, -0.02] {
            units.push(near(1.0, 0.0, 0.0, 0.0, w));
        }
        for w in [0.0, 0.02, -0.02] {
            units.push(near(0.95, 0.31, 0.0, 0.0, w));
        }
        for w in [0.0, 0.02, -0.02] {
            units.push(near(0.0, 0.0, 1.0, 0.0, w));
        }
        let durations = vec![2.0; 9];
        assert_eq!(
            sized_ahc(&units, &durations, 3, 4.0, 0.7),
            [0, 0, 0, 0, 0, 0, 1, 1, 1]
        );
    }

    #[test]
    fn without_a_minimum_it_is_plain_ahc() {
        let (units, durations) = two_voices_and_noise();
        assert_eq!(sized_ahc(&units, &durations, 2, 0.0, 0.7), ahc(&units, 2));
    }

    #[test]
    fn when_every_cluster_is_small_the_plain_split_stays() {
        let (units, _) = two_voices_and_noise();
        let durations = vec![0.1; units.len()];
        assert_eq!(sized_ahc(&units, &durations, 2, 4.0, 0.7), ahc(&units, 2));
        assert!(sized_ahc(&[], &[], 2, 4.0, 0.7).is_empty());
    }
}
