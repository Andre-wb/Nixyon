use super::voice_record::VoiceRecord;

/// Убирает у всех голосов центры с этой встречи. Голос, у которого не осталось ни центров,
/// ни образца, остаётся в библиотеке: его удаляет только сам человек.
pub fn forget_meeting(voices: &mut [VoiceRecord], meeting: &str) {
    for voice in voices {
        voice.centers.retain(|c| c.meeting != meeting);
    }
}

#[cfg(test)]
mod tests {
    use super::super::meeting_center::MeetingCenter;
    use super::*;

    fn center(meeting: &str) -> MeetingCenter {
        MeetingCenter {
            meeting: meeting.into(),
            embedding: vec![0.5],
            remembered_at: "2026-11-07T10:00:00Z".into(),
            voice_model: Some("m.onnx".into()),
        }
    }

    fn voice(name: &str, embedding: Option<Vec<f32>>, meetings: &[&str]) -> VoiceRecord {
        VoiceRecord {
            name: name.into(),
            embedding,
            voice_model: None,
            created_at: "2026-11-07T09:00:00Z".into(),
            centers: meetings.iter().map(|m| center(m)).collect(),
        }
    }

    #[test]
    fn centers_of_the_meeting_go_from_every_voice_and_other_centers_stay() {
        let (gone, kept) = ("2026-11-07T10:00:00Z", "2026-11-07T11:00:00Z");
        let mut voices = vec![
            voice("Анна", Some(vec![1.0]), &[gone, kept]),
            voice("Борис", None, &[kept, gone]),
        ];
        forget_meeting(&mut voices, gone);
        assert_eq!(voices[0].centers, vec![center(kept)]);
        assert_eq!(voices[1].centers, vec![center(kept)]);
        assert_eq!(voices[0].embedding, Some(vec![1.0]));
    }

    #[test]
    fn voice_left_without_centers_and_sample_stays_in_the_library() {
        let gone = "2026-11-07T10:00:00Z";
        let mut voices = vec![voice("Анна", None, &[gone])];
        forget_meeting(&mut voices, gone);
        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].name, "Анна");
        assert!(voices[0].centers.is_empty());
    }
}
