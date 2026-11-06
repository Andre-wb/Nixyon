/// Метки говорящих для последовательности окон: минимум
/// `J = −Σ s[t][y_t] + penalty × (число смен говорящего)`.
///
/// `scores[t][k]`: насколько окно `t` похоже на говорящего `k` (косинус). Все строки
/// одной длины K ≥ 1, иначе ответ пустой. Переход «от любого другого говорящего»
/// всегда выгоднее всего от лучшего на прошлом шаге, поэтому работа T × K, а не T × K².
/// При равной цене остаётся прежний говорящий.
pub fn viterbi(scores: &[Vec<f64>], switch_penalty: f64) -> Vec<usize> {
    let k_n = scores.first().map_or(0, Vec::len);
    if k_n == 0 || scores.iter().any(|row| row.len() != k_n) {
        return Vec::new();
    }
    let mut cost: Vec<f64> = scores[0].iter().map(|s| -s).collect();
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(scores.len());
    back.push((0..k_n).collect());
    for row in &scores[1..] {
        let (best, best_cost) = best_of(&cost);
        let mut next = vec![0.0; k_n];
        let mut from = vec![0usize; k_n];
        for k in 0..k_n {
            let (prev, prev_cost) = if cost[k] <= best_cost + switch_penalty {
                (k, cost[k])
            } else {
                (best, best_cost + switch_penalty)
            };
            next[k] = prev_cost - row[k];
            from[k] = prev;
        }
        cost = next;
        back.push(from);
    }
    let mut labels = vec![0; scores.len()];
    let mut current = best_of(&cost).0;
    for t in (0..scores.len()).rev() {
        labels[t] = current;
        current = back[t][current];
    }
    labels
}

// Номер и цена самого дешёвого; при равенстве меньший номер.
fn best_of(cost: &[f64]) -> (usize, f64) {
    cost.iter().copied().enumerate().fold(
        (0, f64::INFINITY),
        |acc, (i, c)| if c < acc.1 { (i, c) } else { acc },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total_cost(scores: &[Vec<f64>], labels: &[usize], penalty: f64) -> f64 {
        let fit: f64 = labels.iter().enumerate().map(|(t, &k)| -scores[t][k]).sum();
        let switches = labels.windows(2).filter(|w| w[0] != w[1]).count() as f64;
        fit + penalty * switches
    }

    fn brute_force(scores: &[Vec<f64>], penalty: f64) -> f64 {
        let (t_n, k_n) = (scores.len(), scores[0].len());
        let mut best = f64::INFINITY;
        let mut labels = vec![0usize; t_n];
        loop {
            best = best.min(total_cost(scores, &labels, penalty));
            let mut i = 0;
            loop {
                if i == t_n {
                    return best;
                }
                labels[i] += 1;
                if labels[i] < k_n {
                    break;
                }
                labels[i] = 0;
                i += 1;
            }
        }
    }

    #[test]
    fn one_stray_window_is_smoothed_away_by_the_penalty() {
        let s = vec![
            vec![0.9, 0.1],
            vec![0.8, 0.2],
            vec![0.4, 0.5],
            vec![0.9, 0.1],
        ];
        assert_eq!(viterbi(&s, 0.0), [0, 0, 1, 0]);
        assert_eq!(viterbi(&s, 0.2), [0, 0, 0, 0]);
    }

    #[test]
    fn clear_change_of_speaker_survives_the_penalty() {
        let s = vec![
            vec![0.9, 0.1],
            vec![0.9, 0.1],
            vec![0.1, 0.9],
            vec![0.1, 0.9],
        ];
        assert_eq!(viterbi(&s, 0.3), [0, 0, 1, 1]);
    }

    #[test]
    fn matches_brute_force_on_random_sequences() {
        // Простой линейный конгруэнтный генератор: тесту нужны воспроизводимые числа.
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            f64::from(u32::try_from(state >> 44).unwrap()) / f64::from(1u32 << 20) * 2.0 - 1.0
        };
        for case in 0..200 {
            let t_n = 1 + case % 7;
            let k_n = 1 + case % 3;
            let s: Vec<Vec<f64>> = (0..t_n)
                .map(|_| (0..k_n).map(|_| next()).collect())
                .collect();
            let penalty = f64::from(u32::try_from(case % 5).unwrap()) * 0.15;
            let labels = viterbi(&s, penalty);
            assert_eq!(labels.len(), t_n);
            let got = total_cost(&s, &labels, penalty);
            assert!(
                (got - brute_force(&s, penalty)).abs() < 1e-9,
                "{s:?} {penalty}"
            );
        }
    }

    #[test]
    fn empty_or_ragged_input_gives_no_labels() {
        assert!(viterbi(&[], 0.2).is_empty());
        assert!(viterbi(&[vec![]], 0.2).is_empty());
        assert!(viterbi(&[vec![0.1, 0.2], vec![0.3]], 0.2).is_empty());
    }
}
