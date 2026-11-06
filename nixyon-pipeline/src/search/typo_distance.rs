/// Сколько опечаток допускается в слове запроса такой длины (в буквах).
pub(crate) fn allowed_typos(len: usize) -> usize {
    match len {
        0..=3 => 0,
        4..=7 => 1,
        _ => 2,
    }
}

/// Число правок (вставка, пропуск, замена, перестановка соседних букв) между словами,
/// если оно не больше `max`; иначе None.
pub(crate) fn typo_distance(a: &[char], b: &[char], max: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let width = b.len() + 1;
    let mut prev2 = vec![0; width];
    let mut prev: Vec<usize> = (0..width).collect();
    let mut row = vec![0; width];
    for i in 1..=a.len() {
        row[0] = i;
        let mut row_min = row[0];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (prev[j] + 1).min(row[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(prev2[j - 2] + 1);
            }
            row[j] = best;
            row_min = row_min.min(best);
        }
        if row_min > max {
            return None;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut row);
    }
    let distance = prev[b.len()];
    (distance <= max).then_some(distance)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(a: &str, b: &str, max: usize) -> Option<usize> {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        typo_distance(&a, &b, max)
    }

    #[test]
    fn each_kind_of_typo_costs_one() {
        assert_eq!(d("встреча", "встреча", 2), Some(0));
        assert_eq!(d("встерча", "встреча", 2), Some(1));
        assert_eq!(d("встеча", "встреча", 2), Some(1));
        assert_eq!(d("вствреча", "встреча", 2), Some(1));
        assert_eq!(d("встреча", "встрела", 2), Some(1));
    }

    #[test]
    fn distance_above_limit_is_none() {
        assert_eq!(d("встреча", "встать", 1), None);
        assert_eq!(d("а", "абвг", 2), None);
    }

    #[test]
    fn allowed_typos_grow_with_word_length() {
        assert_eq!(
            (allowed_typos(3), allowed_typos(4), allowed_typos(7)),
            (0, 1, 1)
        );
        assert_eq!(allowed_typos(8), 2);
    }
}
