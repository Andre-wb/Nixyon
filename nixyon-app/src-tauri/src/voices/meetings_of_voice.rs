use crate::meetings::MeetingId;

use super::meeting_center::MeetingCenter;
use super::voice_meeting::VoiceMeeting;

/// Встречи архива, с которых у голоса есть центр, новые сверху. Центры встреч, которых в
/// архиве нет, в список не попадают: открыть такую встречу нельзя, и в счётчике её нет.
pub fn meetings_of_voice(
    centers: &[MeetingCenter],
    archive: &[(MeetingId, Option<String>)],
) -> Vec<VoiceMeeting> {
    let mut found: Vec<&(MeetingId, Option<String>)> = archive
        .iter()
        .filter(|(id, _)| centers.iter().any(|c| c.meeting == id.as_str()))
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found
        .into_iter()
        .map(|(id, title)| VoiceMeeting {
            id: id.as_str().to_owned(),
            title: title.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn center(meeting: &str) -> MeetingCenter {
        MeetingCenter {
            meeting: meeting.into(),
            embedding: vec![0.5],
            remembered_at: "2026-11-07T12:00:00Z".into(),
            voice_model: Some("m.onnx".into()),
        }
    }

    fn id(text: &str) -> MeetingId {
        MeetingId::parse(text).unwrap()
    }

    #[test]
    fn meetings_with_a_center_come_newest_first_with_their_titles() {
        let archive = vec![
            (id("2026-11-01T10:00:00Z"), None),
            (id("2026-11-07T10:00:00Z"), Some("Планёрка".to_owned())),
            (id("2026-11-03T10:00:00Z"), None),
        ];
        let centers = [
            center("2026-11-01T10:00:00Z"),
            center("2026-11-07T10:00:00Z"),
        ];
        assert_eq!(
            meetings_of_voice(&centers, &archive),
            vec![
                VoiceMeeting {
                    id: "2026-11-07T10:00:00Z".into(),
                    title: Some("Планёрка".into()),
                },
                VoiceMeeting {
                    id: "2026-11-01T10:00:00Z".into(),
                    title: None,
                },
            ]
        );
    }

    #[test]
    fn center_of_a_meeting_missing_from_the_archive_is_not_listed() {
        let archive = vec![(id("2026-11-07T10:00:00Z"), None)];
        let centers = [
            center("2026-10-01T10:00:00Z"),
            center("2026-11-07T10:00:00Z"),
        ];
        let found = meetings_of_voice(&centers, &archive);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "2026-11-07T10:00:00Z");
        assert!(meetings_of_voice(&[center("2026-10-01T10:00:00Z")], &archive).is_empty());
    }
}
