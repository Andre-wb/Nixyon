/// Окно для оценки громкости: 20 мс при 16 кГц.
pub const WINDOW: usize = 320;
/// Речь: окна не тише самого громкого окна клипа на столько децибел.
pub const BELOW_PEAK_DB: f64 = 20.0;

/// Границы речи в клипе, в сэмплах: от начала первого громкого окна до конца
/// последнего. None, если в клипе одни нули.
pub fn speech_bounds(samples: &[f32]) -> Option<(usize, usize)> {
    let levels: Vec<f64> = samples.chunks(WINDOW).map(rms_db).collect();
    let peak = levels.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !peak.is_finite() {
        return None;
    }
    let loud = |db: &f64| *db >= peak - BELOW_PEAK_DB;
    let first = levels.iter().position(loud)?;
    let last = levels.iter().rposition(loud)?;
    Some((first * WINDOW, ((last + 1) * WINDOW).min(samples.len())))
}

fn rms_db(window: &[f32]) -> f64 {
    let power = window
        .iter()
        .map(|&x| f64::from(x) * f64::from(x))
        .sum::<f64>()
        / window.len() as f64;
    10.0 * power.log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(len: usize, amplitude: f32) -> Vec<f32> {
        (0..len)
            .map(|i| amplitude * if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect()
    }

    #[test]
    fn silence_around_speech_is_cut_to_whole_windows() {
        let mut clip = vec![0.0; WINDOW * 10];
        clip.extend(tone(WINDOW * 5, 0.5));
        clip.extend(vec![0.0; WINDOW * 20]);
        assert_eq!(speech_bounds(&clip), Some((WINDOW * 10, WINDOW * 15)));
    }

    #[test]
    fn quiet_noise_more_than_20_db_below_peak_is_not_speech() {
        let mut clip = tone(WINDOW * 3, 0.005);
        clip.extend(tone(WINDOW * 4, 0.5));
        clip.extend(tone(WINDOW * 3, 0.06));
        assert_eq!(speech_bounds(&clip), Some((WINDOW * 3, WINDOW * 10)));
    }

    #[test]
    fn all_zero_clip_has_no_speech() {
        assert_eq!(speech_bounds(&vec![0.0; WINDOW * 4]), None);
    }

    #[test]
    fn short_last_window_does_not_overrun_the_clip() {
        let clip = tone(WINDOW + 7, 0.5);
        assert_eq!(speech_bounds(&clip), Some((0, WINDOW + 7)));
    }
}
