use std::collections::BTreeMap;

/// Проверенные границы речи клипов: ключ `<сессия>/<файл>.wav`, значение
/// `[начало, конец]` в секундах от начала клипа.
pub struct KnownBounds(BTreeMap<String, [f64; 2]>);

impl KnownBounds {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let map: BTreeMap<String, [f64; 2]> =
            serde_json::from_str(json).map_err(|e| format!("файл границ: {e}"))?;
        if let Some((key, _)) = map
            .iter()
            .find(|(_, [a, b])| !a.is_finite() || !b.is_finite() || *a < 0.0 || a >= b)
        {
            return Err(format!("файл границ: неверные границы у {key}"));
        }
        Ok(Self(map))
    }

    pub fn seconds(&self, session: &str, file: &str) -> Option<(f64, f64)> {
        self.0
            .get(&format!("{session}/{file}"))
            .map(|&[a, b]| (a, b))
    }
}

/// Границы в секундах переводятся в сэмплы; конец не дальше длины клипа.
pub fn to_samples(
    (start, end): (f64, f64),
    rate: u32,
    len: usize,
) -> Result<(usize, usize), String> {
    // Секунды проверены на конечность и неотрицательность при чтении файла.
    let at = |sec: f64| (sec * f64::from(rate)).round() as usize;
    let (a, b) = (at(start), at(end).min(len));
    if a >= b {
        return Err(format!(
            "границы {start}..{end} с вне клипа длиной {len} сэмплов"
        ));
    }
    Ok((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_found_by_session_and_file() {
        let b = KnownBounds::from_json(r#"{"s-1/ru_0001.wav": [0.12, 3.5]}"#).unwrap();
        assert_eq!(b.seconds("s-1", "ru_0001.wav"), Some((0.12, 3.5)));
        assert_eq!(b.seconds("s-2", "ru_0001.wav"), None);
    }

    #[test]
    fn broken_bounds_are_rejected() {
        assert!(KnownBounds::from_json(r#"{"a/b.wav": [2.0, 1.0]}"#).is_err());
        assert!(KnownBounds::from_json(r#"{"a/b.wav": [-0.1, 1.0]}"#).is_err());
        assert!(KnownBounds::from_json(r#"{"a/b.wav": [0.0]}"#).is_err());
    }

    #[test]
    fn seconds_become_samples_and_end_is_clamped_to_clip() {
        assert_eq!(to_samples((0.5, 1.0), 16_000, 32_000), Ok((8_000, 16_000)));
        assert_eq!(to_samples((0.5, 9.0), 16_000, 20_000), Ok((8_000, 20_000)));
        assert!(to_samples((5.0, 9.0), 16_000, 20_000).is_err());
    }
}
