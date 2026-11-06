/// Младше считается ребёнком, с этого возраста взрослым. Между ними подростки: для «ребёнок
/// или взрослый» они не размечены, как группа teens в Common Voice.
const CHILD_BELOW_AGE: u32 = 13;
const ADULT_FROM_AGE: u32 = 19;

/// Ребёнок ли по возрасту в годах; None для подростков.
pub fn child_by_age(age: u32) -> Option<bool> {
    if age < CHILD_BELOW_AGE {
        Some(true)
    } else if age >= ADULT_FROM_AGE {
        Some(false)
    } else {
        None
    }
}

/// Ребёнок ли по возрасту из анкеты: число лет («10») или диапазон («13-17»). Диапазон
/// размечен, только если весь лежит по одну сторону границ. None для подростков и
/// нечитаемого текста.
pub fn child_by_age_text(text: &str) -> Option<bool> {
    let parse = |t: &str| t.trim().parse::<u32>().ok();
    let (low, high) = match text.split_once('-') {
        Some((a, b)) => (parse(a)?, parse(b)?),
        None => {
            let age = parse(text)?;
            (age, age)
        }
    };
    if low > high {
        return None;
    }
    match (child_by_age(low), child_by_age(high)) {
        (Some(a), Some(b)) if a == b => Some(a),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_bounds_split_children_teens_and_adults() {
        assert_eq!(child_by_age(12), Some(true));
        assert_eq!(child_by_age(13), None);
        assert_eq!(child_by_age(18), None);
        assert_eq!(child_by_age(19), Some(false));
    }

    #[test]
    fn age_text_is_a_year_or_a_range_that_lies_on_one_side() {
        assert_eq!(child_by_age_text("10"), Some(true));
        assert_eq!(child_by_age_text("13-17"), None);
        assert_eq!(child_by_age_text("18-19"), None);
        assert_eq!(child_by_age_text("20-29"), Some(false));
        assert_eq!(child_by_age_text("4-12"), Some(true));
        assert_eq!(child_by_age_text("NAN"), None);
        assert_eq!(child_by_age_text("30-20"), None);
    }
}
