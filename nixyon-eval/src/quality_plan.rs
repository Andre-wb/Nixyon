use crate::is_valid_dataset_id;

/// Сколько брать из одного набора: минуты звука или весь набор.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanItem {
    pub dataset: String,
    pub minutes: Option<f64>,
}

/// Разбор `<набор>=<минуты>` или `<набор>=all`. Минуты больше нуля, имя набора как в
/// корне наборов; один набор не повторяется.
pub fn parse_plan_item(raw: &str, earlier: &[PlanItem]) -> Result<PlanItem, String> {
    let (dataset, amount) = raw
        .split_once('=')
        .ok_or_else(|| format!("«{raw}»: нужно <набор>=<минуты> или <набор>=all"))?;
    if !is_valid_dataset_id(dataset) {
        return Err(format!(
            "«{dataset}»: имя набора из строчных латинских букв, цифр и _"
        ));
    }
    if earlier.iter().any(|p| p.dataset == dataset) {
        return Err(format!("набор {dataset} указан дважды"));
    }
    let minutes = if amount == "all" {
        None
    } else {
        Some(
            amount
                .parse::<f64>()
                .ok()
                .filter(|m| m.is_finite() && *m > 0.0)
                .ok_or_else(|| format!("{dataset}: минуты, число больше нуля, или all"))?,
        )
    };
    Ok(PlanItem {
        dataset: dataset.to_owned(),
        minutes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_and_the_whole_set_are_understood() {
        assert_eq!(
            parse_plan_item("cv21_test=90.5", &[]).unwrap(),
            PlanItem {
                dataset: "cv21_test".into(),
                minutes: Some(90.5)
            }
        );
        assert_eq!(parse_plan_item("podlodka=all", &[]).unwrap().minutes, None);
    }

    #[test]
    fn bad_items_are_refused() {
        for raw in [
            "cv",
            "cv=0",
            "cv=-1",
            "cv=NaN",
            "cv=inf",
            "cv=много",
            "../cv=1",
            "CV=1",
            "=1",
        ] {
            assert!(parse_plan_item(raw, &[]).is_err(), "{raw}");
        }
        let first = parse_plan_item("cv=1", &[]).unwrap();
        assert!(parse_plan_item("cv=2", &[first]).is_err());
    }
}
