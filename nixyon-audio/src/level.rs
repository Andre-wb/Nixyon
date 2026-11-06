/// Среднеквадратичный уровень. Для пустого входа 0.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    // Корень из среднего квадрата сэмплов в [-1, 1] не выходит за 1, f32 хватает.
    (sum / samples.len() as f64).sqrt() as f32
}

/// Уровень в дБ относительно полной шкалы. Тишина даёт нижний предел, а не минус бесконечность.
pub fn dbfs(level: f32) -> f32 {
    const FLOOR_DB: f32 = -120.0;
    if level <= 0.0 || !level.is_finite() {
        return FLOOR_DB;
    }
    (20.0 * level.log10()).max(FLOOR_DB)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_of_full_scale_square_wave_is_one() {
        assert_eq!(rms(&[1.0, -1.0, 1.0, -1.0]), 1.0);
    }

    #[test]
    fn rms_of_sine_is_amplitude_over_sqrt_two() {
        let sine: Vec<f32> = (0..16_000)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0).sin())
            .collect();
        assert!((rms(&sine) - 0.5 / 2f32.sqrt()).abs() < 1e-3);
    }

    #[test]
    fn empty_input_and_silence_are_floor_db() {
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(dbfs(rms(&[0.0; 10])), -120.0);
        assert_eq!(dbfs(f32::NAN), -120.0);
    }

    #[test]
    fn half_scale_is_about_minus_six_db() {
        assert!((dbfs(0.5) + 6.02).abs() < 0.01);
        assert_eq!(dbfs(1.0), 0.0);
    }
}
