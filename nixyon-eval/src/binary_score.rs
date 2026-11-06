use serde::Serialize;

/// Пороги уверенности, при которых считается доля решённых случаев и ошибки среди них.
/// Ниже порога ответ «не определено».
pub const CONFIDENCE_THRESHOLDS: [f32; 7] = [0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 0.99];

/// Точность выбора из двух классов, a и b, по вероятности класса a.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BinaryScore {
    pub a: ClassScore,
    pub b: ClassScore,
    pub by_confidence: Vec<ConfidenceRow>,
}

/// Сколько случаев класса и сколько из них угадано, если решать по большей вероятности.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassScore {
    pub name: &'static str,
    pub total: usize,
    pub correct: usize,
}

/// При пороге уверенности: сколько случаев решено и сколько из решённых ошибочно.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConfidenceRow {
    pub threshold: f32,
    pub decided: usize,
    pub errors: usize,
}

impl BinaryScore {
    /// `items`: верно ли, что случай из класса a, и вероятность класса a по модели.
    pub fn new(a: &'static str, b: &'static str, items: &[(bool, f32)]) -> Self {
        let class = |name, is_a: bool| {
            let mine = items.iter().filter(|(truth, _)| *truth == is_a);
            ClassScore {
                name,
                total: mine.clone().count(),
                correct: mine.filter(|(_, p)| (*p >= 0.5) == is_a).count(),
            }
        };
        let by_confidence = CONFIDENCE_THRESHOLDS
            .iter()
            .map(|&threshold| {
                let decided = items.iter().filter(|(_, p)| p.max(1.0 - p) >= threshold);
                ConfidenceRow {
                    threshold,
                    decided: decided.clone().count(),
                    errors: decided.filter(|(truth, p)| (*p >= 0.5) != *truth).count(),
                }
            })
            .collect();
        Self {
            a: class(a, true),
            b: class(b, false),
            by_confidence,
        }
    }

    pub fn total(&self) -> usize {
        self.a.total + self.b.total
    }

    pub fn correct(&self) -> usize {
        self.a.correct + self.b.correct
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_class_counts_its_cases_and_hits_by_the_larger_probability() {
        let s = BinaryScore::new("a", "b", &[(true, 0.9), (true, 0.4), (false, 0.2)]);
        assert_eq!((s.a.total, s.a.correct), (2, 1));
        assert_eq!((s.b.total, s.b.correct), (1, 1));
        assert_eq!((s.total(), s.correct()), (3, 2));
    }

    #[test]
    fn a_higher_threshold_decides_fewer_cases_and_drops_unsure_errors() {
        let s = BinaryScore::new("a", "b", &[(true, 0.97), (true, 0.45), (false, 0.3)]);
        let at = |t: f32| {
            s.by_confidence
                .iter()
                .find(|r| (r.threshold - t).abs() < 1e-6)
                .unwrap()
        };
        assert_eq!((at(0.5).decided, at(0.5).errors), (3, 1));
        assert_eq!((at(0.6).decided, at(0.6).errors), (2, 0));
        assert_eq!((at(0.95).decided, at(0.95).errors), (1, 0));
    }

    #[test]
    fn no_cases_give_zero_counts() {
        let s = BinaryScore::new("a", "b", &[]);
        assert_eq!(s.total(), 0);
        assert!(s.by_confidence.iter().all(|r| r.decided == 0));
    }
}
