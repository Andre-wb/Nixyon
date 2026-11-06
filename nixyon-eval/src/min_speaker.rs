/// Значение флага `--min-speaker-sec`: наименьшее время речи спикера, секунды от 0 до 60;
/// 0 выключает правило.
pub fn min_speaker_sec(value: Option<String>) -> Result<f64, String> {
    value
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| (0.0..=60.0).contains(v))
        .ok_or_else(|| "--min-speaker-sec: секунды от 0 до 60".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_in_range_are_taken_and_the_rest_refused() {
        assert_eq!(min_speaker_sec(Some("4".into())), Ok(4.0));
        assert_eq!(min_speaker_sec(Some("0".into())), Ok(0.0));
        for bad in [
            None,
            Some("-1".into()),
            Some("61".into()),
            Some("NaN".into()),
            Some("a".into()),
        ] {
            assert!(min_speaker_sec(bad).is_err());
        }
    }
}
