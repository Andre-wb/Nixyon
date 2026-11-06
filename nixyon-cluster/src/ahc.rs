use crate::cosine;

/// Агломеративная кластеризация со средней связью по косинусному расстоянию:
/// начинаем с отдельных точек и сливаем две самые близкие группы, пока их не станет `k`.
/// Возвращает номер кластера (0..k) для каждой точки. Номера идут в порядке первого
/// появления, поэтому результат не зависит от того, как сливались группы.
pub fn ahc(units: &[Vec<f32>], k: usize) -> Vec<usize> {
    let n = units.len();
    if n == 0 {
        return Vec::new();
    }
    let k = k.clamp(1, n);
    let mut dist = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let d = 1.0 - cosine(&units[i], &units[j]);
            dist[i][j] = d;
            dist[j][i] = d;
        }
    }
    // members[i]: точки группы i; пустая группа уже слита в другую.
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut alive = n;
    while alive > k {
        let mut best = (f64::INFINITY, 0, 0);
        for i in 0..n {
            if members[i].is_empty() {
                continue;
            }
            for j in (i + 1)..n {
                if !members[j].is_empty() && dist[i][j] < best.0 {
                    best = (dist[i][j], i, j);
                }
            }
        }
        let (_, a, b) = best;
        // Средняя связь, формула Ланса-Уильямса.
        let (na, nb) = (members[a].len() as f64, members[b].len() as f64);
        for c in 0..n {
            if c != a && c != b && !members[c].is_empty() {
                let d = (na * dist[a][c] + nb * dist[b][c]) / (na + nb);
                dist[a][c] = d;
                dist[c][a] = d;
            }
        }
        let moved = std::mem::take(&mut members[b]);
        members[a].extend(moved);
        alive -= 1;
    }
    let mut group_of = vec![0usize; n];
    for (g, m) in members.iter().enumerate() {
        for &p in m {
            group_of[p] = g;
        }
    }
    relabel_by_first_appearance(&group_of)
}

fn relabel_by_first_appearance(groups: &[usize]) -> Vec<usize> {
    let mut map: Vec<(usize, usize)> = Vec::new();
    groups
        .iter()
        .map(|g| match map.iter().find(|(from, _)| from == g) {
            Some(&(_, to)) => to,
            None => {
                let to = map.len();
                map.push((*g, to));
                to
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::blobs;

    #[test]
    fn separates_well_separated_voices() {
        let (units, truth) = blobs(&[5, 7, 4], 0.15, 1);
        assert_eq!(ahc(&units, 3), truth);
    }

    #[test]
    fn labels_follow_first_appearance() {
        let (units, _) = blobs(&[3, 3], 0.1, 2);
        let mut reordered = units[3..].to_vec();
        reordered.extend_from_slice(&units[..3]);
        assert_eq!(ahc(&reordered, 2), [0, 0, 0, 1, 1, 1]);
    }

    #[test]
    fn k_is_clamped_to_one_and_to_number_of_points() {
        let (units, _) = blobs(&[2, 2], 0.1, 3);
        assert_eq!(ahc(&units, 0), [0, 0, 0, 0]);
        assert_eq!(ahc(&units, 99), [0, 1, 2, 3]);
        assert!(ahc(&[], 2).is_empty());
    }
}
