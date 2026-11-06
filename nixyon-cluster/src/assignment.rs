/// Венгерский алгоритм за O(n³): для квадратной матрицы стоимостей находит
/// перестановку с минимальной суммой. Результат: номер столбца для каждой строки.
/// Стоимости должны быть конечными: с NaN или бесконечностью цикл поиска не сходится.
pub fn min_cost_assignment(cost: &[Vec<f64>]) -> Vec<usize> {
    let n = cost.len();
    assert!(
        cost.iter()
            .all(|row| row.len() == n && row.iter().all(|c| c.is_finite())),
        "матрица стоимостей должна быть квадратной и конечной"
    );
    // Индексы с 1: столбец 0 служит фиктивным началом пути увеличения.
    let mut row_potential = vec![0.0; n + 1];
    let mut col_potential = vec![0.0; n + 1];
    let mut row_of_col = vec![0usize; n + 1];
    let mut previous_col = vec![0usize; n + 1];
    for row in 1..=n {
        row_of_col[0] = row;
        let mut col = 0;
        let mut min_slack = vec![f64::INFINITY; n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[col] = true;
            let current_row = row_of_col[col];
            let mut delta = f64::INFINITY;
            let mut next_col = 0;
            for j in 1..=n {
                if used[j] {
                    continue;
                }
                let slack =
                    cost[current_row - 1][j - 1] - row_potential[current_row] - col_potential[j];
                if slack < min_slack[j] {
                    min_slack[j] = slack;
                    previous_col[j] = col;
                }
                if min_slack[j] < delta {
                    delta = min_slack[j];
                    next_col = j;
                }
            }
            for j in 0..=n {
                if used[j] {
                    row_potential[row_of_col[j]] += delta;
                    col_potential[j] -= delta;
                } else {
                    min_slack[j] -= delta;
                }
            }
            col = next_col;
            if row_of_col[col] == 0 {
                break;
            }
        }
        while col != 0 {
            let previous = previous_col[col];
            row_of_col[col] = row_of_col[previous];
            col = previous;
        }
    }
    let mut col_of_row = vec![0; n];
    for (col, &row) in row_of_col.iter().enumerate().skip(1) {
        col_of_row[row - 1] = col - 1;
    }
    col_of_row
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(cost: &[Vec<f64>], assignment: &[usize]) -> f64 {
        assignment
            .iter()
            .enumerate()
            .map(|(r, &c)| cost[r][c])
            .sum()
    }

    fn brute_force_min(cost: &[Vec<f64>]) -> f64 {
        fn go(cost: &[Vec<f64>], row: usize, used: &mut Vec<bool>) -> f64 {
            if row == cost.len() {
                return 0.0;
            }
            let mut best = f64::INFINITY;
            for col in 0..cost.len() {
                if !used[col] {
                    used[col] = true;
                    best = best.min(cost[row][col] + go(cost, row + 1, used));
                    used[col] = false;
                }
            }
            best
        }
        go(cost, 0, &mut vec![false; cost.len()])
    }

    // Простой линейный конгруэнтный генератор: тесту нужны воспроизводимые матрицы.
    fn matrices() -> impl Iterator<Item = Vec<Vec<f64>>> {
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            f64::from(u32::try_from(state >> 40).unwrap() % 100) - 30.0
        };
        (0..300).map(move |i| {
            let n = 1 + i % 6;
            (0..n).map(|_| (0..n).map(|_| next()).collect()).collect()
        })
    }

    #[test]
    fn matches_brute_force_on_many_random_matrices() {
        for cost in matrices() {
            let a = min_cost_assignment(&cost);
            let mut cols = a.clone();
            cols.sort_unstable();
            assert_eq!(cols, (0..cost.len()).collect::<Vec<_>>(), "{cost:?}");
            assert_eq!(total(&cost, &a), brute_force_min(&cost), "{cost:?}");
        }
    }

    #[test]
    fn diagonal_is_kept_when_it_is_cheapest() {
        let cost = vec![vec![0.0, 5.0], vec![5.0, 0.0]];
        assert_eq!(min_cost_assignment(&cost), [0, 1]);
    }

    #[test]
    fn empty_matrix_gives_empty_assignment() {
        assert!(min_cost_assignment(&[]).is_empty());
    }
}
