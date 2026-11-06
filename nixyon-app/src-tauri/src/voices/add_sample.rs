use super::voice_record::VoiceRecord;

/// У голоса с таким именем образец уже есть.
#[derive(Debug, PartialEq)]
pub struct SampleTaken;

/// Кладёт эмбеддинг образца и модель голоса, которой он посчитан, под имя. Имя ищется без
/// учёта регистра. Голос, запомненный только по встречам, получает образец и дату его
/// записи; голоса нет: создаётся новый.
pub fn add_sample(
    voices: &mut Vec<VoiceRecord>,
    name: &str,
    embedding: Vec<f32>,
    voice_model: String,
    created_at: String,
) -> Result<(), SampleTaken> {
    let lower = name.to_lowercase();
    match voices.iter_mut().find(|v| v.name.to_lowercase() == lower) {
        Some(voice) if voice.embedding.is_some() => Err(SampleTaken),
        Some(voice) => {
            voice.embedding = Some(embedding);
            voice.voice_model = Some(voice_model);
            voice.created_at = created_at;
            Ok(())
        }
        None => {
            voices.push(VoiceRecord {
                name: name.to_string(),
                embedding: Some(embedding),
                voice_model: Some(voice_model),
                created_at,
                centers: Vec::new(),
            });
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voices::meeting_center::MeetingCenter;

    fn voice(name: &str, embedding: Option<Vec<f32>>) -> VoiceRecord {
        VoiceRecord {
            name: name.into(),
            embedding,
            voice_model: Some("m.onnx".into()),
            created_at: "2026-10-29T10:00:00Z".into(),
            centers: vec![MeetingCenter {
                meeting: "m1".into(),
                embedding: vec![0.5],
                remembered_at: "2026-10-29T10:00:00Z".into(),
                voice_model: Some("m.onnx".into()),
            }],
        }
    }

    #[test]
    fn voice_known_only_by_meetings_gets_the_sample_and_keeps_its_centers() {
        let mut voices = vec![voice("Анна", None)];
        add_sample(
            &mut voices,
            "анна",
            vec![1.0],
            "new.onnx".into(),
            "2026-10-30T10:00:00Z".into(),
        )
        .unwrap();
        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].name, "Анна");
        assert_eq!(voices[0].embedding, Some(vec![1.0]));
        assert_eq!(voices[0].voice_model.as_deref(), Some("new.onnx"));
        assert_eq!(voices[0].created_at, "2026-10-30T10:00:00Z");
        assert_eq!(voices[0].centers.len(), 1);
    }

    #[test]
    fn voice_with_a_sample_is_not_overwritten() {
        let mut voices = vec![voice("Анна", Some(vec![0.1]))];
        let result = add_sample(&mut voices, "АННА", vec![1.0], "m.onnx".into(), "x".into());
        assert_eq!(result, Err(SampleTaken));
        assert_eq!(voices[0].embedding, Some(vec![0.1]));
    }

    #[test]
    fn new_name_becomes_a_new_voice() {
        let mut voices = vec![voice("Анна", Some(vec![0.1]))];
        add_sample(&mut voices, "Борис", vec![1.0], "m.onnx".into(), "x".into()).unwrap();
        assert_eq!(voices.len(), 2);
        assert_eq!(voices[1].embedding, Some(vec![1.0]));
        assert_eq!(voices[1].voice_model.as_deref(), Some("m.onnx"));
    }
}
