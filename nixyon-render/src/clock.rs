/// Время для чтения человеком: `0:05`, `12:40`, от часа `1:02:03`. Доли секунды
/// отбрасываются, как в плеерах.
pub fn clock(seconds: f64) -> String {
    let total = whole_seconds(seconds);
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Метка времени SRT: `00:01:02,345`, миллисекунды через запятую.
pub fn srt_timestamp(seconds: f64) -> String {
    let ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let (h, m, s, rest) = (
        ms / 3_600_000,
        ms % 3_600_000 / 60_000,
        ms % 60_000 / 1000,
        ms % 1000,
    );
    format!("{h:02}:{m:02}:{s:02},{rest:03}")
}

fn whole_seconds(seconds: f64) -> u64 {
    if seconds.is_finite() && seconds > 0.0 {
        seconds.floor() as u64
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_times_have_no_hours_and_long_ones_do() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(5.9), "0:05");
        assert_eq!(clock(760.0), "12:40");
        assert_eq!(clock(3723.4), "1:02:03");
        assert_eq!(clock(-3.0), "0:00");
        assert_eq!(clock(f64::NAN), "0:00");
    }

    #[test]
    fn srt_timestamp_uses_comma_and_rounds_to_milliseconds() {
        assert_eq!(srt_timestamp(0.0), "00:00:00,000");
        assert_eq!(srt_timestamp(62.3456), "00:01:02,346");
        assert_eq!(srt_timestamp(3723.0005), "01:02:03,001");
        assert_eq!(srt_timestamp(-1.0), "00:00:00,000");
    }
}
