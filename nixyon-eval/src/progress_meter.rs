use std::time::Duration;

/// Как часто и с каким шагом печатается прогресс.
const MIN_INTERVAL: Duration = Duration::from_secs(1);
const MIN_STEP: f64 = 0.001;

/// Строки прогресса для хаба: `@progress <звука готово, с> <звука всего, с> <клипов готово>
/// <клипов всего> <прошло, с>`. Не чаще раза в секунду и только если доля выросла хотя бы
/// на 0,1 %, чтобы журнал не разрастался; последняя строка печатается всегда.
#[derive(Debug)]
pub struct ProgressMeter {
    total_sec: f64,
    total_clips: usize,
    done_sec: f64,
    done_clips: usize,
    shown_share: f64,
    shown_at: Option<Duration>,
}

impl ProgressMeter {
    pub fn new(total_sec: f64, total_clips: usize) -> Self {
        Self {
            total_sec,
            total_clips,
            done_sec: 0.0,
            done_clips: 0,
            shown_share: 0.0,
            shown_at: None,
        }
    }

    /// Учитывает готовый клип; `elapsed`: время с начала прогона. Возвращает строку,
    /// если её пора напечатать.
    pub fn clip_done(&mut self, duration_sec: f64, elapsed: Duration) -> Option<String> {
        self.done_sec += duration_sec;
        self.done_clips += 1;
        let share = if self.total_sec > 0.0 {
            (self.done_sec / self.total_sec).min(1.0)
        } else {
            self.done_clips as f64 / self.total_clips.max(1) as f64
        };
        let last = self.done_clips >= self.total_clips;
        let due = self
            .shown_at
            .is_none_or(|at| elapsed.saturating_sub(at) >= MIN_INTERVAL);
        if !(last || due && share - self.shown_share >= MIN_STEP) {
            return None;
        }
        self.shown_share = share;
        self.shown_at = Some(elapsed);
        Some(format!(
            "@progress {:.1} {:.1} {} {} {:.1}",
            self.done_sec,
            self.total_sec,
            self.done_clips,
            self.total_clips,
            elapsed.as_secs_f64()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(sec: f64) -> Duration {
        Duration::from_secs_f64(sec)
    }

    #[test]
    fn a_line_waits_a_second_and_a_tenth_of_a_percent() {
        let mut meter = ProgressMeter::new(10_000.0, 1_000);
        assert!(meter.clip_done(10.0, at(0.5)).is_some());
        assert!(
            meter.clip_done(10.0, at(0.9)).is_none(),
            "прошло меньше секунды"
        );
        assert!(meter.clip_done(0.1, at(5.0)).is_some());
        assert!(
            meter.clip_done(0.1, at(7.0)).is_none(),
            "доля выросла меньше чем на 0,1 %"
        );
    }

    #[test]
    fn the_last_clip_is_always_shown_and_the_line_has_the_agreed_fields() {
        let mut meter = ProgressMeter::new(3.0, 2);
        meter.clip_done(1.0, at(0.0));
        let line = meter.clip_done(2.0, at(0.2)).unwrap();
        assert_eq!(line, "@progress 3.0 3.0 2 2 0.2");
    }
}
