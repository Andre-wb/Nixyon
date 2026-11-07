use std::path::Path;

use nixyon_android::{AgePiece, AgeSpeaker, SpeakerAges, SpeakerAgesRequest};
use nixyon_core::{SpeakerAge, SpeakerGender, Transcript};
use nixyon_pipeline::{VOICE_PIECES_MAX_SEC, speaker_voice_pieces};

/// Запрос «ребёнок или взрослый»: для каждого спикера его самые длинные реплики. None,
/// если ни у одного спикера нет подходящей речи.
pub fn age_request(wav: &Path, transcript: &Transcript) -> Option<SpeakerAgesRequest> {
    let speakers: Vec<AgeSpeaker> = speaker_voice_pieces(transcript, VOICE_PIECES_MAX_SEC)
        .into_iter()
        .map(|s| AgeSpeaker {
            id: s.speaker,
            pieces: s
                .pieces
                .into_iter()
                .map(|(start_sec, end_sec)| AgePiece { start_sec, end_sec })
                .collect(),
        })
        .collect();
    (!speakers.is_empty()).then(|| SpeakerAgesRequest {
        wav_path: wav.to_string_lossy().into_owned(),
        speakers,
    })
}

/// Метки «ребёнок или взрослый» и пол из ответа модели. Номер, которого нет в
/// расшифровке, пропускается.
pub fn apply_ages(transcript: &mut Transcript, ages: &SpeakerAges) {
    if !ages.available {
        return;
    }
    for verdict in &ages.speakers {
        if let Some(speaker) = transcript.speaker_mut(verdict.id) {
            speaker.age = SpeakerAge::from_verdict(verdict.child);
            speaker.gender = SpeakerGender::from_female(verdict.female);
        }
    }
}

#[cfg(test)]
mod tests {
    use nixyon_android::SpeakerAgeVerdict;
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION, Speaker, Turn};

    use super::*;

    fn transcript(turns: &[(u32, f64, f64)]) -> Transcript {
        let mut ids: Vec<u32> = turns.iter().map(|t| t.0).collect();
        ids.sort_unstable();
        ids.dedup();
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: "2026-11-05T00:00:00Z".into(),
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
            speakers: ids
                .into_iter()
                .map(|id| Speaker {
                    id,
                    name: None,
                    talk_time_sec: 0.0,
                    turns: 0,
                    words: 0,
                    age: None,
                    gender: None,
                })
                .collect(),
            turns: turns
                .iter()
                .zip(1..)
                .map(|(&(speaker, start, end), id)| Turn {
                    id,
                    speaker,
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
    fn the_request_carries_each_speaker_with_the_pieces_of_their_speech() {
        let t = transcript(&[(1, 0.0, 4.0), (2, 4.0, 6.0), (1, 6.0, 7.0)]);
        let request = age_request(Path::new("/c/a.wav"), &t).unwrap();
        assert_eq!(request.wav_path, "/c/a.wav");
        assert_eq!(
            request.speakers,
            [
                AgeSpeaker {
                    id: 1,
                    pieces: vec![
                        AgePiece {
                            start_sec: 0.0,
                            end_sec: 4.0
                        },
                        AgePiece {
                            start_sec: 6.0,
                            end_sec: 7.0
                        },
                    ],
                },
                AgeSpeaker {
                    id: 2,
                    pieces: vec![AgePiece {
                        start_sec: 4.0,
                        end_sec: 6.0
                    }],
                },
            ]
        );
    }

    #[test]
    fn without_speech_long_enough_there_is_no_request() {
        let t = transcript(&[(1, 0.0, 0.4)]);
        assert!(age_request(Path::new("a.wav"), &t).is_none());
    }

    #[test]
    fn verdicts_become_ages_and_an_unknown_speaker_is_skipped() {
        let mut t = transcript(&[(1, 0.0, 2.0), (2, 2.0, 4.0), (3, 4.0, 6.0)]);
        let ages = SpeakerAges {
            available: true,
            speakers: vec![
                SpeakerAgeVerdict {
                    id: 1,
                    child: Some(true),
                    female: Some(false),
                },
                SpeakerAgeVerdict {
                    id: 2,
                    child: Some(false),
                    female: Some(true),
                },
                SpeakerAgeVerdict {
                    id: 3,
                    child: None,
                    female: None,
                },
                SpeakerAgeVerdict {
                    id: 9,
                    child: Some(true),
                    female: None,
                },
            ],
        };
        apply_ages(&mut t, &ages);
        let got: Vec<Option<SpeakerAge>> = t.speakers.iter().map(|s| s.age).collect();
        assert_eq!(
            got,
            [Some(SpeakerAge::Child), Some(SpeakerAge::Adult), None]
        );
        let genders: Vec<Option<SpeakerGender>> = t.speakers.iter().map(|s| s.gender).collect();
        assert_eq!(
            genders,
            [Some(SpeakerGender::Male), Some(SpeakerGender::Female), None]
        );
    }

    #[test]
    fn without_the_model_nothing_changes() {
        let mut t = transcript(&[(1, 0.0, 2.0)]);
        let before = t.clone();
        apply_ages(
            &mut t,
            &SpeakerAges {
                available: false,
                speakers: vec![SpeakerAgeVerdict {
                    id: 1,
                    child: Some(true),
                    female: Some(true),
                }],
            },
        );
        assert_eq!(t, before);
    }
}
