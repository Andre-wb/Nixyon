use crate::{CoreError, SCHEMA_VERSION, Transcript};

pub fn to_json(transcript: &Transcript) -> Result<String, CoreError> {
    Ok(serde_json::to_string_pretty(transcript)?)
}

/// Читает расшифровку и отказывается от файлов другой версии формата,
/// чтобы не показать их молча с потерянными полями.
pub fn from_json(text: &str) -> Result<Transcript, CoreError> {
    let transcript: Transcript = serde_json::from_str(text)?;
    if transcript.schema_version != SCHEMA_VERSION {
        return Err(CoreError::UnsupportedVersion {
            found: transcript.schema_version,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(transcript)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioInfo, EngineInfo, Speaker, Turn, Word};

    fn sample() -> Transcript {
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: "2026-10-28T11:05:00Z".into(),
            audio: AudioInfo {
                file: "rec.wav".into(),
                duration_sec: 9.84,
                sample_rate: 16_000,
            },
            engine: EngineInfo {
                vad: "silero_vad.onnx".into(),
                asr: "giga-am-v3".into(),
                embedding: "campplus".into(),
            },
            language: crate::SpeechLanguage::Ru,
            speakers: vec![
                Speaker {
                    id: 1,
                    name: None,
                    talk_time_sec: 4.36,
                    turns: 1,
                    words: 2,
                    age: None,
                    gender: None,
                },
                Speaker {
                    id: 2,
                    name: Some("Анна «Ё» \"кавычки\" \\ \u{1F600}".into()),
                    talk_time_sec: 3.12,
                    turns: 1,
                    words: 3,
                    age: None,
                    gender: None,
                },
            ],
            turns: vec![
                Turn {
                    id: 1,
                    speaker: 1,
                    start: 0.32,
                    end: 4.68,
                    text: "добрый день".into(),
                    words: Some(vec![
                        Word {
                            text: "добрый".into(),
                            start: 0.32,
                            end: 0.76,
                        },
                        Word {
                            text: "день".into(),
                            start: 0.76,
                            end: 1.2,
                        },
                    ]),
                    flags: vec![],
                    emotion: None,
                },
                Turn {
                    id: 2,
                    speaker: 2,
                    start: 5.4,
                    end: 8.52,
                    text: "да я готова".into(),
                    words: None,
                    flags: vec!["clipping".into()],
                    emotion: None,
                },
            ],
        }
    }

    #[test]
    fn transcript_survives_json_round_trip_unchanged() {
        let original = sample();
        let back = from_json(&to_json(&original).unwrap()).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn awkward_floats_survive_json_round_trip_bit_for_bit() {
        // Значения, на которых неточный разбор чисел теряет последний бит.
        let values = [
            0.1 + 0.2,
            1.0 / 3.0,
            2.0_f64.sqrt() * 1e-7,
            3_599.999_999_999_999,
            f64::MIN_POSITIVE,
            123_456.789_012_345,
        ];
        let mut transcript = sample();
        for (i, v) in values.iter().enumerate() {
            transcript.turns[0].start = *v;
            transcript.turns[0].end = v * 2.0;
            let back = from_json(&to_json(&transcript).unwrap()).unwrap();
            assert_eq!(back.turns[0].start.to_bits(), v.to_bits(), "значение {i}");
            assert_eq!(
                back.turns[0].end.to_bits(),
                (v * 2.0).to_bits(),
                "значение {i}"
            );
        }
    }

    #[test]
    fn name_null_is_kept_as_null() {
        let json = to_json(&sample()).unwrap();
        assert!(json.contains("\"name\": null"));
    }

    #[test]
    fn turn_without_flags_field_reads_as_empty_flags() {
        let mut value: serde_json::Value =
            serde_json::from_str(&to_json(&sample()).unwrap()).unwrap();
        value["turns"][1].as_object_mut().unwrap().remove("flags");
        let back = from_json(&value.to_string()).unwrap();
        assert!(back.turns[1].flags.is_empty());
    }

    #[test]
    fn transcript_without_language_reads_as_russian_and_english_survives() {
        let mut value: serde_json::Value =
            serde_json::from_str(&to_json(&sample()).unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("language");
        let old = from_json(&value.to_string()).unwrap();
        assert_eq!(old.language, crate::SpeechLanguage::Ru);
        let mut english = sample();
        english.language = crate::SpeechLanguage::En;
        let json = to_json(&english).unwrap();
        assert!(json.contains("\"language\": \"en\""), "{json}");
        assert_eq!(
            from_json(&json).unwrap().language,
            crate::SpeechLanguage::En
        );
    }

    #[test]
    fn a_speaker_without_an_age_reads_and_writes_without_the_field() {
        let json = to_json(&sample()).unwrap();
        assert!(!json.contains("\"age\""));
        assert!(!json.contains("\"gender\""));
        let back = from_json(&json).unwrap();
        assert!(back.speakers.iter().all(|s| s.age.is_none()));
    }

    #[test]
    fn a_speaker_age_survives_json_round_trip() {
        let mut transcript = sample();
        transcript.speakers[0].age = Some(crate::SpeakerAge::Child);
        transcript.speakers[0].gender = Some(crate::SpeakerGender::Female);
        let json = to_json(&transcript).unwrap();
        assert!(json.contains("\"age\": \"child\""));
        assert!(json.contains("\"gender\": \"female\""));
        assert_eq!(from_json(&json).unwrap(), transcript);
    }

    #[test]
    fn other_schema_version_is_rejected() {
        let mut transcript = sample();
        transcript.schema_version = 2;
        let err = from_json(&to_json(&transcript).unwrap()).unwrap_err();
        assert!(matches!(
            err,
            CoreError::UnsupportedVersion {
                found: 2,
                supported: 1
            }
        ));
    }

    #[test]
    fn broken_or_incomplete_json_is_an_error_not_a_panic() {
        for text in ["", "{", "[]", "{\"schema_version\": 1}", "null"] {
            assert!(
                matches!(from_json(text), Err(CoreError::Parse(_))),
                "{text:?}"
            );
        }
    }

    #[test]
    fn times_are_numbers_not_strings() {
        let value: serde_json::Value = serde_json::from_str(&to_json(&sample()).unwrap()).unwrap();
        assert!(value["turns"][0]["start"].is_f64());
        assert!(value["turns"][0]["words"][0]["end"].is_f64());
    }
}
