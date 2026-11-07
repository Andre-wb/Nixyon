use serde::Serialize;

/// Встреча архива, на которой голос узнан. `title`: своё название; None, и интерфейс
/// называет встречу датой и временем.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VoiceMeeting {
    pub id: String,
    pub title: Option<String>,
}
