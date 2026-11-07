//! Граница автоочистки звука в календарных днях пояса устройства: звук хранится у встреч
//! с местной полуночи дня «сегодня минус N», у более ранних удаляется. Пояс приходит из
//! интерфейса смещением от UTC в минутах; id встречи в UTC, поэтому граница тоже
//! переводится в id и сравнивается с ним как строка.

use super::meeting_id::MeetingId;

const DAY_SECS: i64 = 86_400;
/// Самые далёкие от UTC пояса: от −12:00 до +14:00, берётся с запасом в обе стороны.
const MAX_OFFSET_MINUTES: i32 = 14 * 60;

pub fn valid_offset(offset_minutes: i32) -> bool {
    (-MAX_OFFSET_MINUTES..=MAX_OFFSET_MINUTES).contains(&offset_minutes)
}

/// Первая секунда, звук с которой ещё хранится, в виде id встречи. `offset_minutes`:
/// на сколько минут местное время впереди UTC.
pub fn cutoff(now_secs: u64, offset_minutes: i32, days: u32) -> MeetingId {
    let offset = i64::from(offset_minutes) * 60;
    let now = i64::try_from(now_secs).unwrap_or(i64::MAX - offset.abs());
    let local_day = (now + offset).div_euclid(DAY_SECS);
    let midnight = (local_day - i64::from(days)) * DAY_SECS - offset;
    MeetingId::from_unix_secs(u64::try_from(midnight).unwrap_or(0))
}

/// Встречи, чей звук старше границы.
pub fn older_than<'a>(
    ids: impl IntoIterator<Item = &'a MeetingId>,
    cutoff: &MeetingId,
) -> Vec<MeetingId> {
    ids.into_iter().filter(|id| *id < cutoff).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOSCOW: i32 = 180;
    const NEW_YORK_WINTER: i32 = -300;

    // Секунды UNIX из id: обратная к `MeetingId::from_unix_secs`, только для тестов.
    fn secs(id: &str) -> u64 {
        let n = |range: std::ops::Range<usize>| id[range].parse::<i64>().unwrap();
        let (y, m, d) = (n(0..4), n(5..7), n(8..10));
        let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        u64::try_from(days * DAY_SECS + n(11..13) * 3600 + n(14..16) * 60 + n(17..19)).unwrap()
    }

    fn id(text: &str) -> MeetingId {
        MeetingId::parse(text).unwrap()
    }

    #[test]
    fn border_is_local_midnight_of_today_minus_n_days_in_moscow() {
        let border = cutoff(secs("2026-11-07T10:00:00Z"), MOSCOW, 7);
        assert_eq!(border.as_str(), "2026-10-30T21:00:00Z");
        let ids = [
            id("2026-10-30T20:59:59Z"),
            id("2026-10-30T21:00:00Z"),
            id("2026-11-07T09:00:00Z"),
        ];
        assert_eq!(older_than(&ids, &border), vec![ids[0].clone()]);
    }

    #[test]
    fn local_date_decides_the_day_not_the_utc_date() {
        // В Нью-Йорке ещё 6 ноября, хотя по UTC уже 7-е.
        let border = cutoff(secs("2026-11-07T03:00:00Z"), NEW_YORK_WINTER, 7);
        assert_eq!(border.as_str(), "2026-10-30T05:00:00Z");
        // В Москве в 23:30 по UTC уже следующий день.
        let border = cutoff(secs("2026-11-06T23:30:00Z"), MOSCOW, 7);
        assert_eq!(border.as_str(), "2026-10-30T21:00:00Z");
    }

    #[test]
    fn exactly_at_local_midnight_today_already_counts() {
        let border = cutoff(secs("2026-11-06T21:00:00Z"), MOSCOW, 1);
        assert_eq!(border.as_str(), "2026-11-05T21:00:00Z");
        let border = cutoff(secs("2026-11-06T20:59:59Z"), MOSCOW, 1);
        assert_eq!(border.as_str(), "2026-11-04T21:00:00Z");
    }

    #[test]
    fn border_crosses_new_year_and_february_29() {
        let border = cutoff(secs("2027-01-03T00:30:00Z"), MOSCOW, 7);
        assert_eq!(border.as_str(), "2026-12-26T21:00:00Z");
        let border = cutoff(secs("2028-03-01T12:00:00Z"), 0, 1);
        assert_eq!(border.as_str(), "2028-02-29T00:00:00Z");
    }

    #[test]
    fn border_before_1970_keeps_every_meeting() {
        let border = cutoff(3_600, MOSCOW, 90);
        assert_eq!(border.as_str(), "1970-01-01T00:00:00Z");
        assert!(older_than(&[id("1970-01-01T00:00:00Z")], &border).is_empty());
    }

    #[test]
    fn offset_beyond_real_time_zones_is_rejected() {
        assert!(valid_offset(MOSCOW) && valid_offset(-720) && valid_offset(840));
        assert!(!valid_offset(841) && !valid_offset(-841) && !valid_offset(i32::MIN));
    }
}
