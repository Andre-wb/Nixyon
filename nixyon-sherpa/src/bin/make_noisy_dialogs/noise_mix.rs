/// Наибольшая амплитуда смеси: громче смесь целиком уменьшается, ОСШ не меняется.
const PEAK: f32 = 0.99;

/// Накладывает шум на запись с заданным ОСШ в дБ. Мощность речи считается только на
/// участках речи (`spans`, отсчёты от и до), чтобы паузы между репликами не занижали её.
/// Шум повторяется по кругу до длины записи, его мощность считается по той части, что
/// легла на запись: у неровного шума громкость остальной части в запись не попадает.
/// None: нет речи или шум пустой либо беззвучный.
pub fn add_noise(
    speech: &[f32],
    spans: &[(usize, usize)],
    noise: &[f32],
    snr_db: f64,
) -> Option<Vec<f32>> {
    let speech_power = power(
        spans
            .iter()
            .flat_map(|&(from, to)| &speech[from.min(speech.len())..to.min(speech.len())]),
    )?;
    if noise.is_empty() {
        return None;
    }
    let noise_power = power(noise.iter().cycle().take(speech.len()))?;
    if noise_power == 0.0 {
        return None;
    }
    let gain = (speech_power / noise_power / 10f64.powf(snr_db / 10.0)).sqrt();
    // Множитель шума в разумных пределах, приведение к f32 без заметной потери.
    let gain = gain as f32;
    let mut mixed: Vec<f32> = speech
        .iter()
        .zip(noise.iter().cycle())
        .map(|(s, n)| s + n * gain)
        .collect();
    let peak = mixed.iter().fold(0f32, |m, x| m.max(x.abs()));
    if peak > PEAK {
        let scale = PEAK / peak;
        mixed.iter_mut().for_each(|x| *x *= scale);
    }
    Some(mixed)
}

// Средняя мощность; None для пустого набора.
fn power<'a>(samples: impl Iterator<Item = &'a f32>) -> Option<f64> {
    let (sum, count) = samples.fold((0.0, 0usize), |(sum, count), &x| {
        (sum + f64::from(x) * f64::from(x), count + 1)
    });
    // Число отсчётов далеко от 2^53, перевод точный.
    (count > 0).then(|| sum / count as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snr_db(speech: &[f32], mixed: &[f32], span: (usize, usize)) -> f64 {
        let s = power(speech[span.0..span.1].iter()).unwrap();
        let noise: Vec<f32> = mixed.iter().zip(speech).map(|(m, s)| m - s).collect();
        10.0 * (s / power(noise.iter()).unwrap()).log10()
    }

    #[test]
    fn noise_gets_the_asked_ratio_to_speech_and_pauses_do_not_count() {
        // Речь только в первой половине, вторая половина тишина.
        let speech: Vec<f32> = (0..1000)
            .map(|i| {
                if i < 500 {
                    0.3 * ((i % 7) as f32 - 3.0) / 3.0
                } else {
                    0.0
                }
            })
            .collect();
        let noise: Vec<f32> = (0..77).map(|i| ((i % 5) as f32 - 2.0) / 2.0).collect();
        let mixed = add_noise(&speech, &[(0, 500)], &noise, 10.0).unwrap();
        assert_eq!(mixed.len(), speech.len());
        assert!((snr_db(&speech, &mixed, (0, 500)) - 10.0).abs() < 0.2);
    }

    #[test]
    fn only_the_noise_that_lands_on_the_recording_counts() {
        // Шум длиннее записи, и его громкая часть в запись не попадает.
        let speech: Vec<f32> = (0..400)
            .map(|i| 0.3 * ((i % 7) as f32 - 3.0) / 3.0)
            .collect();
        let noise: Vec<f32> = (0..1000)
            .map(|i| {
                let level = if i < 400 { 0.05 } else { 0.5 };
                level * ((i % 5) as f32 - 2.0) / 2.0
            })
            .collect();
        let mixed = add_noise(&speech, &[(0, 400)], &noise, 10.0).unwrap();
        assert!((snr_db(&speech, &mixed, (0, 400)) - 10.0).abs() < 0.2);
    }

    #[test]
    fn loud_mix_is_scaled_down_whole_and_keeps_the_ratio() {
        let speech = vec![0.9f32, -0.9, 0.9, -0.9];
        let noise = vec![0.5f32, 0.5, -0.5, -0.5];
        let mixed = add_noise(&speech, &[(0, 4)], &noise, 0.0).unwrap();
        assert!(mixed.iter().all(|x| x.abs() <= PEAK + 1e-6));
        // При 0 дБ шум получает ту же мощность, что речь: первый отсчёт до уменьшения
        // 0,9 + 0,9, отсюда общий множитель уменьшения.
        let scale = mixed[0] / (0.9 + 0.9);
        let scaled: Vec<f32> = speech.iter().map(|s| s * scale).collect();
        assert!((snr_db(&scaled, &mixed, (0, 4)) - 0.0).abs() < 0.2);
    }

    #[test]
    fn silent_noise_or_no_speech_is_refused() {
        assert_eq!(add_noise(&[0.1, 0.2], &[(0, 2)], &[0.0, 0.0], 10.0), None);
        assert_eq!(add_noise(&[0.1, 0.2], &[(0, 2)], &[], 10.0), None);
        assert_eq!(add_noise(&[0.1, 0.2], &[], &[0.5], 10.0), None);
    }
}
