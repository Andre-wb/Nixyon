/// Расстояние Левенштейна: минимум замен, вставок и удалений, чтобы из `a` получить `b`.
/// Память на одну строку таблицы.
pub fn edit_distance<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if x == y {
                diagonal
            } else {
                1 + diagonal.min(above).min(row[j])
            };
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn classic_kitten_sitting_needs_three_edits() {
        assert_eq!(edit_distance(&chars("kitten"), &chars("sitting")), 3);
    }

    #[test]
    fn empty_side_costs_length_of_the_other() {
        assert_eq!(edit_distance(&chars(""), &chars("abc")), 3);
        assert_eq!(edit_distance(&chars("abc"), &chars("")), 3);
        assert_eq!(edit_distance::<char>(&[], &[]), 0);
    }

    #[test]
    fn substitution_insertion_and_deletion_each_cost_one() {
        let a = ["я", "иду", "домой"];
        assert_eq!(edit_distance(&a, &["я", "еду", "домой"]), 1);
        assert_eq!(edit_distance(&a, &["я", "иду", "домой", "сейчас"]), 1);
        assert_eq!(edit_distance(&a, &["я", "домой"]), 1);
        assert_eq!(edit_distance(&a, &a), 0);
    }
}
