/// Сроки автоочистки звука в днях, из которых выбирают в настройках.
pub const CLEANUP_DAYS: [u32; 3] = [7, 30, 90];

/// None: очистка выключена; иначе только срок из списка.
pub fn valid(days: Option<u32>) -> bool {
    days.is_none_or(|d| CLEANUP_DAYS.contains(&d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_terms_and_switched_off_are_accepted() {
        assert!(valid(None));
        for days in CLEANUP_DAYS {
            assert!(valid(Some(days)));
        }
        for days in [0, 1, 8, 365, u32::MAX] {
            assert!(!valid(Some(days)), "{days}");
        }
    }
}
