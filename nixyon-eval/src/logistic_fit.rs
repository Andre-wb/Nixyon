use nixyon_ort::{ChildAdult, ChildAdultFeatures};

/// Признак с разбросом меньше считается постоянным: масштаб 1, вклада в ответ почти нет.
const MIN_SCALE: f64 = 1e-6;
const MAX_STEPS: usize = 5000;
/// Обучение останавливается, когда длина градиента меньше.
const GRAD_TOLERANCE: f64 = 1e-6;
const POWER_STEPS: usize = 60;

/// Логистическая регрессия «ребёнок или взрослый» с L2-штрафом: признаки стандартизуются по
/// обучающим клипам, веса подбираются ускоренным градиентным спуском (Нестеров с
/// перезапуском, шаг по оценке константы Липшица). Сдвиг не штрафуется. Результат зависит
/// только от входа. `rows`: признаки клипов, `child`: класс, `weights`: вес клипа.
/// Порог уверенности у результата 0,5: его выбирают отдельно.
pub fn fit_logistic(
    features: ChildAdultFeatures,
    rows: &[&[f32]],
    child: &[bool],
    weights: &[f64],
    l2: f64,
) -> Result<ChildAdult, String> {
    let n = rows.len();
    let dim = rows.first().map_or(0, |r| r.len());
    if n == 0 || dim == 0 || child.len() != n || weights.len() != n {
        return Err("нет клипов для обучения или длины не совпадают".into());
    }
    if rows.iter().any(|r| r.len() != dim) {
        return Err("у клипов разная длина признаков".into());
    }
    let total: f64 = weights.iter().sum();
    if total <= 0.0 || !total.is_finite() || !(l2 >= 0.0 && l2.is_finite()) {
        return Err("веса клипов или штраф неверные".into());
    }
    let w: Vec<f64> = weights.iter().map(|w| w / total).collect();
    let (mean, scale) = standardization(rows, dim);
    let x: Vec<f64> = rows
        .iter()
        .flat_map(|r| {
            r.iter()
                .zip(&mean)
                .zip(&scale)
                .map(|((v, m), s)| (f64::from(*v) - m) / s)
        })
        .collect();
    let y: Vec<f64> = child.iter().map(|c| f64::from(u8::from(*c))).collect();
    let problem = Problem {
        x: &x,
        y: &y,
        w: &w,
        dim,
        l2,
    };
    let beta = problem.minimize();
    Ok(ChildAdult {
        features,
        mean: mean.iter().map(|m| *m as f32).collect(),
        scale: scale.iter().map(|s| *s as f32).collect(),
        weights: beta[..dim].iter().map(|b| *b as f32).collect(),
        bias: beta[dim] as f32,
        confidence: 0.5,
    })
}

fn standardization(rows: &[&[f32]], dim: usize) -> (Vec<f64>, Vec<f64>) {
    let n = rows.len() as f64;
    let mut mean = vec![0.0; dim];
    for r in rows {
        for (m, v) in mean.iter_mut().zip(r.iter()) {
            *m += f64::from(*v);
        }
    }
    mean.iter_mut().for_each(|m| *m /= n);
    let mut var = vec![0.0; dim];
    for r in rows {
        for ((s, v), m) in var.iter_mut().zip(r.iter()).zip(&mean) {
            *s += (f64::from(*v) - m).powi(2);
        }
    }
    let scale = var
        .into_iter()
        .map(|s| {
            let sd = (s / n).sqrt();
            if sd < MIN_SCALE { 1.0 } else { sd }
        })
        .collect();
    (mean, scale)
}

/// Взвешенная логистическая потеря на стандартизованных признаках. Последний элемент
/// вектора параметров: сдвиг.
struct Problem<'a> {
    x: &'a [f64],
    y: &'a [f64],
    w: &'a [f64],
    dim: usize,
    l2: f64,
}

impl Problem<'_> {
    fn row(&self, i: usize) -> &[f64] {
        &self.x[i * self.dim..(i + 1) * self.dim]
    }

    fn margin(&self, beta: &[f64], i: usize) -> f64 {
        self.row(i)
            .iter()
            .zip(beta)
            .map(|(a, b)| a * b)
            .sum::<f64>()
            + beta[self.dim]
    }

    fn loss(&self, beta: &[f64]) -> f64 {
        let data: f64 = (0..self.y.len())
            .map(|i| {
                let z = self.margin(beta, i);
                // log(1 + e^z) без переполнения при большом z.
                let softplus = z.max(0.0) + (-z.abs()).exp().ln_1p();
                self.w[i] * (softplus - self.y[i] * z)
            })
            .sum();
        data + 0.5 * self.l2 * beta[..self.dim].iter().map(|b| b * b).sum::<f64>()
    }

    fn gradient(&self, beta: &[f64]) -> Vec<f64> {
        let mut g = vec![0.0; self.dim + 1];
        for i in 0..self.y.len() {
            let z = self.margin(beta, i);
            let r = self.w[i] * (1.0 / (1.0 + (-z).exp()) - self.y[i]);
            for (gj, xj) in g.iter_mut().zip(self.row(i)) {
                *gj += r * xj;
            }
            g[self.dim] += r;
        }
        for (gj, bj) in g.iter_mut().zip(beta).take(self.dim) {
            *gj += self.l2 * bj;
        }
        g
    }

    /// Константа Липшица градиента: четверть наибольшего собственного числа взвешенной
    /// матрицы признаков (со столбцом единиц) плюс штраф, с запасом.
    fn lipschitz(&self) -> f64 {
        let mut v = vec![1.0; self.dim + 1];
        let mut eigen = 1.0;
        for _ in 0..POWER_STEPS {
            let mut next = vec![0.0; self.dim + 1];
            for i in 0..self.y.len() {
                let row = self.row(i);
                let xv = row.iter().zip(&v).map(|(a, b)| a * b).sum::<f64>() + v[self.dim];
                let r = self.w[i] * xv;
                for (nj, xj) in next.iter_mut().zip(row) {
                    *nj += r * xj;
                }
                next[self.dim] += r;
            }
            let norm = next.iter().map(|a| a * a).sum::<f64>().sqrt();
            if norm <= f64::EPSILON {
                break;
            }
            eigen = norm / v.iter().map(|a| a * a).sum::<f64>().sqrt();
            v = next.into_iter().map(|a| a / norm).collect();
        }
        1.1 * (0.25 * eigen + self.l2)
    }

    fn minimize(&self) -> Vec<f64> {
        let step = 1.0 / self.lipschitz();
        let mut beta = vec![0.0; self.dim + 1];
        let mut previous = beta.clone();
        let mut momentum_step = 0usize;
        let mut last_loss = self.loss(&beta);
        for _ in 0..MAX_STEPS {
            let k = momentum_step as f64;
            let look: Vec<f64> = beta
                .iter()
                .zip(&previous)
                .map(|(b, p)| b + k / (k + 3.0) * (b - p))
                .collect();
            let g = self.gradient(&look);
            if g.iter().map(|a| a * a).sum::<f64>().sqrt() < GRAD_TOLERANCE {
                beta = look;
                break;
            }
            let next: Vec<f64> = look.iter().zip(&g).map(|(b, gj)| b - step * gj).collect();
            let loss = self.loss(&next);
            if loss > last_loss {
                // Перезапуск: импульс увёл в сторону роста потери.
                momentum_step = 0;
                previous = beta.clone();
                continue;
            }
            previous = std::mem::replace(&mut beta, next);
            last_loss = loss;
            momentum_step += 1;
        }
        beta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separable_clips_are_classified_on_the_right_side() {
        let rows: Vec<Vec<f32>> = vec![
            vec![1.0, 0.3],
            vec![1.2, -0.1],
            vec![-1.0, 0.2],
            vec![-0.8, -0.4],
        ];
        let refs: Vec<&[f32]> = rows.iter().map(|r| r.as_slice()).collect();
        let child = [true, true, false, false];
        let m = fit_logistic(
            ChildAdultFeatures::Redimnet2,
            &refs,
            &child,
            &[1.0; 4],
            0.01,
        )
        .unwrap();
        for (r, c) in refs.iter().zip(child) {
            let p = m.child_probability(r).unwrap();
            assert_eq!(p >= 0.5, c, "{p}");
        }
        assert!(m.child_probability(&[2.0, 0.0]).unwrap() > 0.9);
    }

    #[test]
    fn the_fit_reaches_the_known_optimum_of_a_one_feature_problem() {
        // Без штрафа и с равными весами при x = ±1 и метках 3:1 на каждой стороне оптимум
        // даёт p(x = 1) = 3/4 и p(x = -1) = 1/4.
        let rows: Vec<Vec<f32>> = [1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0]
            .iter()
            .map(|v| vec![*v])
            .collect();
        let refs: Vec<&[f32]> = rows.iter().map(|r| r.as_slice()).collect();
        let child = [true, true, true, false, false, false, false, true];
        let m = fit_logistic(ChildAdultFeatures::Redimnet2, &refs, &child, &[1.0; 8], 0.0).unwrap();
        assert!((m.child_probability(&[1.0]).unwrap() - 0.75).abs() < 1e-4);
        assert!((m.child_probability(&[-1.0]).unwrap() - 0.25).abs() < 1e-4);
    }

    #[test]
    fn a_heavier_clip_pulls_the_answer_towards_its_class() {
        let rows: Vec<Vec<f32>> = vec![vec![0.0], vec![0.0]];
        let refs: Vec<&[f32]> = rows.iter().map(|r| r.as_slice()).collect();
        let m = fit_logistic(
            ChildAdultFeatures::Redimnet2,
            &refs,
            &[true, false],
            &[3.0, 1.0],
            0.0,
        )
        .unwrap();
        assert!((m.child_probability(&[0.0]).unwrap() - 0.75).abs() < 1e-4);
    }

    #[test]
    fn empty_or_mismatched_input_is_an_error() {
        assert!(fit_logistic(ChildAdultFeatures::Redimnet2, &[], &[], &[], 0.1).is_err());
        let row: &[f32] = &[1.0];
        assert!(
            fit_logistic(
                ChildAdultFeatures::Redimnet2,
                &[row],
                &[true, false],
                &[1.0],
                0.1
            )
            .is_err()
        );
        assert!(fit_logistic(ChildAdultFeatures::Redimnet2, &[row], &[true], &[0.0], 0.1).is_err());
    }
}
