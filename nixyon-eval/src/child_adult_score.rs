use serde::Serialize;

/// Шаг сетки порогов уверенности, из которой выбирается порог.
const CONFIDENCE_STEP: f32 = 0.005;
const MAX_CONFIDENCE: f32 = 0.995;

/// Ответы классификатора «ребёнок или взрослый» по классам при одном пороге уверенности:
/// случай с max(p, 1 - p) ниже порога остаётся «не определено».
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ChildAdultScore {
    pub confidence: f32,
    pub children: usize,
    pub children_found: usize,
    pub children_as_adult: usize,
    pub adults: usize,
    pub adults_found: usize,
    pub adults_as_child: usize,
}

impl ChildAdultScore {
    /// `items`: ребёнок ли на самом деле и вероятность ребёнка по классификатору.
    pub fn new(items: &[(bool, f32)], confidence: f32) -> Self {
        let mut s = Self {
            confidence,
            children: 0,
            children_found: 0,
            children_as_adult: 0,
            adults: 0,
            adults_found: 0,
            adults_as_child: 0,
        };
        for (child, p) in items {
            let decided = p.max(1.0 - p) >= confidence;
            let says_child = *p >= 0.5;
            if *child {
                s.children += 1;
                if decided && says_child {
                    s.children_found += 1;
                } else if decided {
                    s.children_as_adult += 1;
                }
            } else {
                s.adults += 1;
                if decided && !says_child {
                    s.adults_found += 1;
                } else if decided {
                    s.adults_as_child += 1;
                }
            }
        }
        s
    }

    /// Доля взрослых, названных детьми; None, если взрослых нет.
    pub fn adults_as_child_share(&self) -> Option<f64> {
        share(self.adults_as_child, self.adults)
    }

    pub fn children_found_share(&self) -> Option<f64> {
        share(self.children_found, self.children)
    }

    /// Порог приёмки: взрослых названо детьми не больше `max_adults_as_child`, детей
    /// найдено не меньше `min_children_found`.
    pub fn passes(&self, max_adults_as_child: f64, min_children_found: f64) -> bool {
        self.adults_as_child_share()
            .is_some_and(|s| s <= max_adults_as_child)
            && self
                .children_found_share()
                .is_some_and(|s| s >= min_children_found)
    }
}

fn share(part: usize, total: usize) -> Option<f64> {
    (total > 0).then(|| part as f64 / total as f64)
}

/// Наименьший порог уверенности из сетки 0,5..0,995 с шагом 0,005, при котором взрослых
/// названо детьми не больше `max_adults_as_child`. None, если взрослых нет или порога нет.
pub fn choose_confidence(items: &[(bool, f32)], max_adults_as_child: f64) -> Option<f32> {
    let steps = ((MAX_CONFIDENCE - 0.5) / CONFIDENCE_STEP).round() as usize;
    (0..=steps)
        .map(|i| 0.5 + i as f32 * CONFIDENCE_STEP)
        .find(|c| {
            ChildAdultScore::new(items, *c)
                .adults_as_child_share()
                .is_some_and(|s| s <= max_adults_as_child)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: [(bool, f32); 5] = [
        (true, 0.95),
        (true, 0.6),
        (true, 0.2),
        (false, 0.1),
        (false, 0.7),
    ];

    #[test]
    fn each_class_counts_found_wrong_and_undecided() {
        let s = ChildAdultScore::new(&ITEMS, 0.65);
        assert_eq!(
            (s.children, s.children_found, s.children_as_adult),
            (3, 1, 1)
        );
        assert_eq!((s.adults, s.adults_found, s.adults_as_child), (2, 1, 1));
    }

    #[test]
    fn the_chosen_confidence_is_the_lowest_that_keeps_adults_safe() {
        assert_eq!(choose_confidence(&ITEMS, 0.0), Some(0.705));
        assert_eq!(choose_confidence(&ITEMS, 0.5), Some(0.5));
        assert_eq!(choose_confidence(&[(true, 0.9)], 0.0), None);
    }

    #[test]
    fn acceptance_needs_both_safe_adults_and_found_children() {
        let s = ChildAdultScore::new(&ITEMS, 0.705);
        assert!(!s.passes(0.02, 0.8));
        assert!(ChildAdultScore::new(&[(true, 0.9), (false, 0.1)], 0.8).passes(0.02, 0.8));
    }
}
