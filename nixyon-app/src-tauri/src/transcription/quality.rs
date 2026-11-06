use nixyon_audio::{QualityIssue, QualityReport};

use super::command_error::CommandError;

/// Запись короче секунды или тишина не обрабатываются; клиппинг не мешает обработке,
/// но интерфейс о нём предупреждает.
pub fn check_quality(report: &QualityReport) -> Result<Vec<&'static str>, CommandError> {
    let issues = report.issues();
    if issues.contains(&QualityIssue::TooShort) {
        return Err(CommandError::new(CommandError::TOO_SHORT));
    }
    if issues.contains(&QualityIssue::Silent) {
        return Err(CommandError::new(CommandError::SILENT));
    }
    Ok(issues
        .contains(&QualityIssue::Clipping)
        .then_some("clipping")
        .into_iter()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(duration_sec: f64, loudest_window_dbfs: f32, clipped_ratio: f64) -> QualityReport {
        QualityReport {
            duration_sec,
            loudest_window_dbfs,
            clipped_ratio,
        }
    }

    #[test]
    fn short_and_silent_records_are_refused_and_clipping_only_warns() {
        assert_eq!(check_quality(&report(10.0, -12.0, 0.0)), Ok(vec![]));
        assert_eq!(
            check_quality(&report(10.0, -12.0, 0.05)),
            Ok(vec!["clipping"])
        );
        assert_eq!(
            check_quality(&report(0.5, -12.0, 0.05)).unwrap_err().code,
            CommandError::TOO_SHORT
        );
        assert_eq!(
            check_quality(&report(10.0, -80.0, 0.0)).unwrap_err().code,
            CommandError::SILENT
        );
    }
}
