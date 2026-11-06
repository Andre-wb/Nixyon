use nixyon_core::Transcript;
use nixyon_metrics::SpeakerSpan;

/// Паузы внутри реплики до этой длины считаются речью: так размечены и эталоны.
pub const SPEECH_GAP_SEC: f64 = 0.5;

/// Отрезки речи для DER по словам реплик: слова одного говорящего склеиваются, если
/// пауза между ними не больше `max_gap`. Реплика целиком включает и длинные паузы, тогда
/// DER засчитывает их как ложную речь. Реплика без времени слов берётся целиком.
pub fn speech_spans(transcript: &Transcript, max_gap: f64) -> Vec<SpeakerSpan> {
    let mut spans = Vec::new();
    for turn in &transcript.turns {
        let words = turn.words.as_deref().unwrap_or_default();
        if words.is_empty() {
            spans.push(SpeakerSpan {
                speaker: turn.speaker,
                start: turn.start,
                end: turn.end,
            });
            continue;
        }
        let mut current: Option<SpeakerSpan> = None;
        for word in words {
            match current.as_mut() {
                Some(span) if word.start - span.end <= max_gap => span.end = span.end.max(word.end),
                _ => {
                    spans.extend(current.take());
                    current = Some(SpeakerSpan {
                        speaker: turn.speaker,
                        start: word.start,
                        end: word.end,
                    });
                }
            }
        }
        spans.extend(current);
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use nixyon_core::{AudioInfo, EngineInfo, Turn, Word};

    fn word(start: f64, end: f64) -> Word {
        Word {
            text: "раз".into(),
            start,
            end,
        }
    }

    fn turn(speaker: u32, start: f64, end: f64, words: Option<Vec<Word>>) -> Turn {
        Turn {
            id: 0,
            speaker,
            start,
            end,
            text: "раз".into(),
            words,
            flags: Vec::new(),
        }
    }

    fn transcript(turns: Vec<Turn>) -> Transcript {
        Transcript {
            schema_version: 1,
            created_at: String::new(),
            audio: AudioInfo {
                file: "a".into(),
                duration_sec: 10.0,
                sample_rate: 16_000,
            },
            engine: EngineInfo {
                vad: String::new(),
                asr: String::new(),
                embedding: String::new(),
            },
            speakers: Vec::new(),
            turns,
        }
    }

    #[test]
    fn long_pauses_inside_a_turn_are_not_speech() {
        let t = transcript(vec![turn(
            1,
            0.0,
            4.0,
            Some(vec![word(0.0, 1.0), word(1.3, 2.0), word(3.5, 4.0)]),
        )]);
        let spans = speech_spans(&t, SPEECH_GAP_SEC);
        assert_eq!(
            spans,
            [
                SpeakerSpan {
                    speaker: 1,
                    start: 0.0,
                    end: 2.0
                },
                SpeakerSpan {
                    speaker: 1,
                    start: 3.5,
                    end: 4.0
                },
            ]
        );
    }

    #[test]
    fn a_turn_without_word_times_is_taken_whole() {
        let t = transcript(vec![turn(2, 1.0, 3.0, None)]);
        assert_eq!(
            speech_spans(&t, SPEECH_GAP_SEC),
            [SpeakerSpan {
                speaker: 2,
                start: 1.0,
                end: 3.0
            }]
        );
    }
}
