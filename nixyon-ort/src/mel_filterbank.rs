// Шкала мел по Слейни, как в librosa (htk=False), которой пользуется NeMo.
const F_SP: f64 = 200.0 / 3.0;
const MIN_LOG_HZ: f64 = 1000.0;
const MIN_LOG_MEL: f64 = MIN_LOG_HZ / F_SP;
// ln(6,4) / 27: шаг логарифмической части шкалы.
const LOG_STEP: f64 = 0.068_751_777_420_949_12;

fn hz_to_mel(hz: f64) -> f64 {
    if hz < MIN_LOG_HZ {
        hz / F_SP
    } else {
        MIN_LOG_MEL + (hz / MIN_LOG_HZ).ln() / LOG_STEP
    }
}

fn mel_to_hz(mel: f64) -> f64 {
    if mel < MIN_LOG_MEL {
        mel * F_SP
    } else {
        MIN_LOG_HZ * ((mel - MIN_LOG_MEL) * LOG_STEP).exp()
    }
}

/// Треугольные фильтры мел от 0 до половины частоты с нормировкой Слейни, как
/// `librosa.filters.mel(norm="slaney")`. Строки: полосы, столбцы: бины БПФ.
pub fn mel_filterbank(n_fft: usize, n_mels: usize, sample_rate: u32) -> Vec<Vec<f32>> {
    let bins = n_fft / 2 + 1;
    let rate = f64::from(sample_rate);
    let (lo, hi) = (hz_to_mel(0.0), hz_to_mel(rate / 2.0));
    let points: Vec<f64> = (0..n_mels + 2)
        .map(|i| mel_to_hz(lo + (hi - lo) * i as f64 / (n_mels + 1) as f64))
        .collect();
    let freqs: Vec<f64> = (0..bins).map(|k| k as f64 * rate / n_fft as f64).collect();
    (0..n_mels)
        .map(|m| {
            let (left, center, right) = (points[m], points[m + 1], points[m + 2]);
            let norm = 2.0 / (right - left);
            freqs
                .iter()
                .map(|&f| {
                    let up = (f - left) / (center - left);
                    let down = (right - f) / (right - center);
                    (up.min(down).max(0.0) * norm) as f32
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_is_linear_below_1_khz_and_round_trips() {
        assert!((hz_to_mel(500.0) - 7.5).abs() < 1e-9);
        for hz in [0.0, 300.0, 1000.0, 4000.0, 8000.0] {
            assert!((mel_to_hz(hz_to_mel(hz)) - hz).abs() < 1e-6);
        }
    }

    #[test]
    fn every_band_is_a_non_negative_triangle_with_some_weight() {
        let fb = mel_filterbank(512, 128, 16_000);
        assert_eq!(fb.len(), 128);
        assert!(fb.iter().all(|band| band.len() == 257));
        assert!(fb.iter().all(|band| band.iter().all(|&w| w >= 0.0)));
        assert!(fb.iter().all(|band| band.iter().any(|&w| w > 0.0)));
    }
}
