use serde::Serialize;

use super::meeting_hit::MeetingHit;

/// Найденное по архиву. `truncated`: совпадений больше, чем показано.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MeetingResults {
    pub hits: Vec<MeetingHit>,
    pub truncated: bool,
}
