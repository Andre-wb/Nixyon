use super::meeting_center::MeetingCenter;
use super::voice_record::VoiceRecord;

/// Сколько центров встреч хранится у человека; самые старые вытесняются. Взято с запасом,
/// замером не проверялось.
pub const MAX_MEETING_CENTERS: usize = 20;

/// Запоминает центр голоса под именем. Имя ищется без учёта регистра; если голоса нет,
/// создаётся голос без образца. Центр с той же встречи заменяется.
pub fn remember(voices: &mut Vec<VoiceRecord>, name: &str, center: MeetingCenter) {
    let lower = name.to_lowercase();
    let index = match voices.iter().position(|v| v.name.to_lowercase() == lower) {
        Some(i) => i,
        None => {
            voices.push(VoiceRecord {
                name: name.to_string(),
                embedding: None,
                voice_model: None,
                created_at: center.remembered_at.clone(),
                centers: Vec::new(),
            });
            voices.len() - 1
        }
    };
    let centers = &mut voices[index].centers;
    centers.retain(|c| c.meeting != center.meeting);
    centers.push(center);
    let extra = centers.len().saturating_sub(MAX_MEETING_CENTERS);
    centers.drain(..extra);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn center(meeting: &str, x: f32) -> MeetingCenter {
        MeetingCenter {
            meeting: meeting.into(),
            embedding: vec![x],
            remembered_at: "2026-10-30T10:00:00Z".into(),
            voice_model: Some("m.onnx".into()),
        }
    }

    fn sampled(name: &str) -> VoiceRecord {
        VoiceRecord {
            name: name.into(),
            embedding: Some(vec![1.0]),
            voice_model: Some("m.onnx".into()),
            created_at: "2026-10-29T10:00:00Z".into(),
            centers: Vec::new(),
        }
    }

    #[test]
    fn center_goes_to_the_voice_with_that_name_regardless_of_case() {
        let mut voices = vec![sampled("Анна")];
        remember(&mut voices, "анна", center("m1", 0.5));
        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].centers.len(), 1);
        assert_eq!(voices[0].embedding, Some(vec![1.0]));
    }

    #[test]
    fn unknown_name_becomes_a_voice_without_sample() {
        let mut voices = vec![sampled("Анна")];
        remember(&mut voices, "Борис", center("m1", 0.5));
        assert_eq!(voices.len(), 2);
        assert_eq!(voices[1].name, "Борис");
        assert_eq!(voices[1].embedding, None);
        assert_eq!(voices[1].centers.len(), 1);
    }

    #[test]
    fn same_meeting_replaces_its_center() {
        let mut voices = Vec::new();
        remember(&mut voices, "Анна", center("m1", 0.5));
        remember(&mut voices, "Анна", center("m1", 0.7));
        assert_eq!(voices[0].centers.len(), 1);
        assert_eq!(voices[0].centers[0].embedding, vec![0.7]);
    }

    #[test]
    fn oldest_centers_are_dropped_above_the_limit() {
        let mut voices = Vec::new();
        for i in 0..=MAX_MEETING_CENTERS {
            remember(&mut voices, "Анна", center(&format!("m{i}"), i as f32));
        }
        let centers = &voices[0].centers;
        assert_eq!(centers.len(), MAX_MEETING_CENTERS);
        assert_eq!(centers[0].meeting, "m1");
        assert_eq!(
            centers[MAX_MEETING_CENTERS - 1].meeting,
            format!("m{MAX_MEETING_CENTERS}")
        );
    }
}
