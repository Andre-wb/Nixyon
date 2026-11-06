/// Медианный фильтр с нечётным окном `window`; у краёв окно укорачивается. Окно 1 и
/// меньше оставляет значения как есть.
pub fn median_filter(values: &[f32], window: usize) -> Vec<f32> {
    if window <= 1 {
        return values.to_vec();
    }
    let half = window / 2;
    let mut buf = Vec::with_capacity(window);
    (0..values.len())
        .map(|i| {
            let from = i.saturating_sub(half);
            let to = (i + half + 1).min(values.len());
            buf.clear();
            buf.extend_from_slice(&values[from..to]);
            buf.sort_by(f32::total_cmp);
            buf[buf.len() / 2]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_spike_is_removed() {
        assert_eq!(median_filter(&[0.0, 0.0, 1.0, 0.0, 0.0], 3), [0.0; 5]);
    }

    #[test]
    fn window_of_one_keeps_values() {
        assert_eq!(median_filter(&[0.3, 0.9], 1), [0.3, 0.9]);
    }

    #[test]
    fn edges_use_a_shorter_window() {
        assert_eq!(median_filter(&[1.0, 0.0, 0.0], 3), [1.0, 0.0, 0.0]);
    }
}
