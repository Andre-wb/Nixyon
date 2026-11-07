use serde::Serialize;

/// Совпадение в архиве для главного экрана. `title`: своё название встречи или `None`,
/// и интерфейс называет её датой из `meeting`. `text`: весь текст реплики, фрагмент вокруг
/// `start` и `end` (единицы UTF-16) вырезает интерфейс.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeetingHit {
    pub meeting: String,
    pub title: Option<String>,
    pub turn_id: u32,
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub exact: bool,
}
