use nalgebra::{DMatrix, SymmetricEigen};

use crate::{ahc, centroids, cosine};

/// Группы, центры которых похожи сильнее этого, считаются одним человеком.
/// Без этого eigengap делит одного человека на части по сессиям записи
/// (другой микрофон или комната) и не отличает «один спикер» от «нескольких».
pub const SAME_SPEAKER_CENTROID_COSINE: f64 = 0.6;
/// Доля соседей, которые остаются в графе похожести у каждой точки.
const NEIGHBOUR_SHARE: f64 = 0.25;
const MIN_NEIGHBOURS: usize = 2;

/// Оценка числа спикеров: скачок собственных значений нормированного лапласиана
/// графа похожести, затем слияние групп с похожими центрами.
/// Результат в пределах 1..=min(max_speakers, число точек). `same_speaker_cosine`:
/// группы с центрами похожее этого сливаются (обычно `SAME_SPEAKER_CENTROID_COSINE`).
pub fn estimate_speaker_count(
    units: &[Vec<f32>],
    max_speakers: usize,
    same_speaker_cosine: f64,
) -> usize {
    let n = units.len();
    if n <= 1 {
        return n;
    }
    let max_k = max_speakers.clamp(1, n);
    if max_k == 1 {
        return 1;
    }
    let by_eigengap = if n == 2 || max_k == 2 {
        2
    } else {
        eigengap(units, max_k.min(n - 1))
    };
    merge_similar_groups(units, by_eigengap, same_speaker_cosine)
}

fn eigengap(units: &[Vec<f32>], max_k: usize) -> usize {
    let laplacian = normalized_laplacian(&prune(&similarity(units)));
    let mut eigenvalues: Vec<f64> = SymmetricEigen::new(laplacian)
        .eigenvalues
        .iter()
        .copied()
        .collect();
    eigenvalues.sort_by(f64::total_cmp);
    // k самых малых собственных значений близки к нулю, если в графе k групп;
    // число групп там, где следующий скачок вверх самый большой.
    (2..=max_k)
        .max_by(|&a, &b| {
            let gap_a = eigenvalues[a] - eigenvalues[a - 1];
            let gap_b = eigenvalues[b] - eigenvalues[b - 1];
            gap_a.total_cmp(&gap_b).then(b.cmp(&a))
        })
        .unwrap_or(2)
}

// Уменьшает число групп, пока центры каких-то двух групп похожи на порог и больше.
fn merge_similar_groups(units: &[Vec<f32>], mut k: usize, same_speaker_cosine: f64) -> usize {
    while k > 1 && max_centroid_cosine(units, k) >= same_speaker_cosine {
        k -= 1;
    }
    k
}

fn max_centroid_cosine(units: &[Vec<f32>], k: usize) -> f64 {
    let centers = centroids(units, &ahc(units, k), k);
    let mut max = f64::NEG_INFINITY;
    for (i, a) in centers.iter().enumerate() {
        for b in &centers[i + 1..] {
            if let (Some(a), Some(b)) = (a, b) {
                max = max.max(cosine(a, b));
            }
        }
    }
    max
}

fn similarity(units: &[Vec<f32>]) -> DMatrix<f64> {
    let n = units.len();
    DMatrix::from_fn(n, n, |i, j| {
        if i == j {
            0.0
        } else {
            cosine(&units[i], &units[j]).max(0.0)
        }
    })
}

// У каждой точки остаются только самые похожие соседи: так слабые случайные
// сходства между разными голосами не склеивают группы. Матрица снова симметризуется.
fn prune(sim: &DMatrix<f64>) -> DMatrix<f64> {
    let n = sim.nrows();
    let keep = ((n as f64 * NEIGHBOUR_SHARE).round() as usize)
        .max(MIN_NEIGHBOURS)
        .min(n - 1);
    let mut pruned = DMatrix::zeros(n, n);
    for i in 0..n {
        let mut order: Vec<usize> = (0..n).filter(|&j| j != i).collect();
        order.sort_by(|&a, &b| sim[(i, b)].total_cmp(&sim[(i, a)]));
        for &j in order.iter().take(keep) {
            pruned[(i, j)] = sim[(i, j)];
        }
    }
    (&pruned + pruned.transpose()) * 0.5
}

fn normalized_laplacian(a: &DMatrix<f64>) -> DMatrix<f64> {
    let n = a.nrows();
    let inv_sqrt_degree: Vec<f64> = (0..n)
        .map(|i| {
            let d: f64 = a.row(i).sum();
            if d > 0.0 { 1.0 / d.sqrt() } else { 0.0 }
        })
        .collect();
    DMatrix::from_fn(n, n, |i, j| {
        let identity = if i == j { 1.0 } else { 0.0 };
        identity - inv_sqrt_degree[i] * a[(i, j)] * inv_sqrt_degree[j]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{blobs, voices_with_sessions};

    #[test]
    fn finds_number_of_distinct_voices() {
        for (sizes, seed) in [
            (vec![6, 6], 1),
            (vec![5, 8, 4], 2),
            (vec![4, 4, 4, 4], 3),
            (vec![10, 3], 4),
        ] {
            let (units, _) = blobs(&sizes, 0.2, seed);
            assert_eq!(
                estimate_speaker_count(&units, 8, SAME_SPEAKER_CENTROID_COSINE),
                sizes.len(),
                "{sizes:?}"
            );
        }
    }

    #[test]
    fn one_voice_gives_one() {
        let (units, _) = blobs(&[12], 0.2, 5);
        assert_eq!(
            estimate_speaker_count(&units, 8, SAME_SPEAKER_CENTROID_COSINE),
            1
        );
    }

    #[test]
    fn result_respects_max_and_number_of_points() {
        let (units, _) = blobs(&[3, 3, 3, 3], 0.1, 6);
        assert!(estimate_speaker_count(&units, 2, SAME_SPEAKER_CENTROID_COSINE) <= 2);
        assert_eq!(
            estimate_speaker_count(&units, 1, SAME_SPEAKER_CENTROID_COSINE),
            1
        );
        assert_eq!(
            estimate_speaker_count(&units[..1], 8, SAME_SPEAKER_CENTROID_COSINE),
            1
        );
        assert_eq!(
            estimate_speaker_count(&[], 8, SAME_SPEAKER_CENTROID_COSINE),
            0
        );
    }

    #[test]
    fn one_voice_recorded_in_two_sessions_is_still_one_speaker() {
        for seed in 1..=5 {
            let (units, _) = voices_with_sessions(2, 2, 4, 0.6, 0.3, seed);
            assert_eq!(
                estimate_speaker_count(&units, 8, SAME_SPEAKER_CENTROID_COSINE),
                2,
                "seed {seed}"
            );
        }
    }

    #[test]
    fn two_points_of_different_voices_give_two() {
        let (units, _) = blobs(&[1, 1], 0.05, 7);
        assert_eq!(
            estimate_speaker_count(&units, 8, SAME_SPEAKER_CENTROID_COSINE),
            2
        );
    }
}
