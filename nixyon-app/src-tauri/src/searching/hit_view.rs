use nixyon_pipeline::{MatchKind, SearchHit};
use serde::Serialize;

/// Найденное место для интерфейса: отрезок текста хода в единицах UTF-16.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HitView {
    pub turn_id: u32,
    pub start: usize,
    pub end: usize,
    pub exact: bool,
}

impl From<&SearchHit> for HitView {
    fn from(h: &SearchHit) -> Self {
        HitView {
            turn_id: h.turn_id,
            start: h.start,
            end: h.end,
            exact: h.kind == MatchKind::Exact,
        }
    }
}
