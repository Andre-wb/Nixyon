use std::path::Path;

use nixyon_android::{EmotionTurn, TurnEmotions, TurnEmotionsRequest};
use nixyon_core::Transcript;

/// Запрос эмоций: все реплики расшифровки. None, если реплик нет.
pub fn emotion_request(wav: &Path, transcript: &Transcript) -> Option<TurnEmotionsRequest> {
    let turns: Vec<EmotionTurn> = transcript
        .turns
        .iter()
        .map(|t| EmotionTurn {
            id: t.id,
            start_sec: t.start,
            end_sec: t.end,
        })
        .collect();
    (!turns.is_empty()).then(|| TurnEmotionsRequest {
        wav_path: wav.to_string_lossy().into_owned(),
        turns,
    })
}

/// Эмоции реплик из ответа модели. Номер, которого нет в расшифровке, пропускается.
pub fn apply_emotions(transcript: &mut Transcript, emotions: &TurnEmotions) {
    if !emotions.available {
        return;
    }
    for verdict in &emotions.turns {
        if let Some(turn) = transcript.turns.iter_mut().find(|t| t.id == verdict.id) {
            turn.emotion = verdict.emotion;
        }
    }
}

#[cfg(test)]
mod tests {
    use nixyon_android::TurnEmotionVerdict;
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION, SpeechLanguage, Turn, TurnEmotion};

    use super::*;

    fn transcript(turns: &[(f64, f64)]) -> Transcript {
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: "2026-11-07T00:00:00Z".into(),
            audio: AudioInfo {
                file: "a.wav".into(),
                duration_sec: 60.0,
                sample_rate: 16_000,
            },
            engine: EngineInfo {
                vad: "vad".into(),
                asr: "asr".into(),
                embedding: "emb".into(),
            },
            language: SpeechLanguage::Ru,
            speakers: Vec::new(),
            turns: turns
                .iter()
                .zip(1..)
                .map(|(&(start, end), id)| Turn {
                    id,
                    speaker: 1,
                    start,
                    end,
                    text: "текст".into(),
                    words: None,
                    flags: Vec::new(),
                    emotion: None,
                })
                .collect(),
        }
    }

    #[test]
    fn the_request_carries_every_turn_with_its_bounds() {
        let t = transcript(&[(0.0, 4.0), (4.5, 6.0)]);
        let request = emotion_request(Path::new("/c/a.wav"), &t).unwrap();
        assert_eq!(request.wav_path, "/c/a.wav");
        assert_eq!(
            request.turns,
            [
                EmotionTurn {
                    id: 1,
                    start_sec: 0.0,
                    end_sec: 4.0
                },
                EmotionTurn {
                    id: 2,
                    start_sec: 4.5,
                    end_sec: 6.0
                },
            ]
        );
    }

    #[test]
    fn without_turns_there_is_no_request() {
        assert!(emotion_request(Path::new("a.wav"), &transcript(&[])).is_none());
    }

    #[test]
    fn verdicts_become_turn_emotions_and_an_unknown_turn_is_skipped() {
        let mut t = transcript(&[(0.0, 2.0), (2.0, 4.0)]);
        apply_emotions(
            &mut t,
            &TurnEmotions {
                available: true,
                turns: vec![
                    TurnEmotionVerdict {
                        id: 1,
                        emotion: Some(TurnEmotion::Anger),
                    },
                    TurnEmotionVerdict {
                        id: 2,
                        emotion: None,
                    },
                    TurnEmotionVerdict {
                        id: 9,
                        emotion: Some(TurnEmotion::Joy),
                    },
                ],
            },
        );
        let got: Vec<Option<TurnEmotion>> = t.turns.iter().map(|x| x.emotion).collect();
        assert_eq!(got, [Some(TurnEmotion::Anger), None]);
    }

    #[test]
    fn without_the_model_nothing_changes() {
        let mut t = transcript(&[(0.0, 2.0)]);
        let before = t.clone();
        apply_emotions(
            &mut t,
            &TurnEmotions {
                available: false,
                turns: vec![TurnEmotionVerdict {
                    id: 1,
                    emotion: Some(TurnEmotion::Sadness),
                }],
            },
        );
        assert_eq!(t, before);
    }

    #[test]
    fn the_plugin_answer_reads_emotion_keys_and_nulls() {
        let answer: TurnEmotions = serde_json::from_str(
            r#"{"available": true, "turns": [{"id": 1, "emotion": "joy"}, {"id": 2, "emotion": null}]}"#,
        )
        .unwrap();
        assert_eq!(answer.turns[0].emotion, Some(TurnEmotion::Joy));
        assert_eq!(answer.turns[1].emotion, None);
    }
}
