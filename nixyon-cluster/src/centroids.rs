use crate::{cosine, unit};

/// Центр каждого кластера: средний вектор, приведённый к единичной длине.
/// У пустого кластера центра нет.
pub fn centroids(units: &[Vec<f32>], labels: &[usize], k: usize) -> Vec<Option<Vec<f32>>> {
    let dim = units.first().map_or(0, Vec::len);
    let mut sums = vec![vec![0.0f32; dim]; k];
    for (v, &label) in units.iter().zip(labels) {
        if label < k {
            for (s, x) in sums[label].iter_mut().zip(v) {
                *s += x;
            }
        }
    }
    sums.iter().map(|s| unit(s)).collect()
}

/// Номер ближайшего по косинусу центра. None, если центров нет.
pub fn nearest(centroids: &[Option<Vec<f32>>], u: &[f32]) -> Option<usize> {
    centroids
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.as_ref().map(|c| (i, cosine(c, u))))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::blobs;

    #[test]
    fn point_goes_to_its_own_voice_centroid() {
        let (units, truth) = blobs(&[6, 6, 6], 0.15, 4);
        let c = centroids(&units, &truth, 3);
        for (u, &t) in units.iter().zip(&truth) {
            assert_eq!(nearest(&c, u), Some(t));
        }
    }

    #[test]
    fn empty_cluster_has_no_centroid_and_is_never_nearest() {
        let (units, _) = blobs(&[3], 0.1, 5);
        let c = centroids(&units, &[0, 0, 0], 2);
        assert!(c[0].is_some() && c[1].is_none());
        assert_eq!(nearest(&c, &units[0]), Some(0));
        assert_eq!(nearest(&[None, None], &units[0]), None);
    }
}
