use nixyon_core::Transcript;
use nixyon_metrics::{
    DER_COLLAR_SEC, MetricsError, char_errors, cp_word_errors, diarization_errors, naming_score,
    overlap_detection, word_errors,
};

use crate::{
    RecordScore, Truth, hypothesis_named_spans, hypothesis_overlaps, hypothesis_spans,
    hypothesis_text, hypothesis_utterances,
};

/// Прогон одной записи: что получилось и сколько это заняло.
pub struct RecordRun<'a> {
    pub name: &'a str,
    pub truth: &'a Truth,
    pub transcript: &'a Transcript,
    pub audio_sec: f64,
    pub processing_sec: f64,
    /// Имена людей, чьи образцы голоса давали конвейеру; None: образцов не было.
    pub samples: Option<&'a [String]>,
}

pub fn score_record(run: &RecordRun<'_>) -> Result<RecordScore, MetricsError> {
    let reference = run.truth.spans();
    let der = diarization_errors(
        &reference,
        &hypothesis_spans(run.transcript),
        DER_COLLAR_SEC,
    )?;
    let text = hypothesis_text(run.transcript);
    let reference_text = run.truth.text();
    Ok(RecordScore {
        name: run.name.to_string(),
        truth_speakers: run.truth.speakers,
        found_speakers: u32::try_from(run.transcript.speakers.len()).unwrap_or(u32::MAX),
        audio_sec: run.audio_sec,
        processing_sec: run.processing_sec,
        der,
        wer: reference_text.as_deref().map(|r| word_errors(r, &text)),
        cer: reference_text.as_deref().map(|r| char_errors(r, &text)),
        cpwer: run
            .truth
            .utterances()
            .map(|r| cp_word_errors(&r, &hypothesis_utterances(run.transcript))),
        overlap: overlap_detection(&reference, &hypothesis_overlaps(run.transcript))?,
        naming: run.samples.map(|names| {
            naming_score(
                &run.truth.named_spans(),
                &hypothesis_named_spans(run.transcript),
                names,
            )
        }),
    })
}

#[cfg(test)]
mod tests {
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION, Speaker, SpeechLanguage, Turn};
    use nixyon_metrics::ErrorCount;

    use super::*;

    fn turn(speaker: u32, start: f64, end: f64, text: &str) -> Turn {
        Turn {
            id: 0,
            speaker,
            start,
            end,
            text: text.into(),
            words: None,
            flags: Vec::new(),
            emotion: None,
        }
    }

    fn transcript(turns: Vec<Turn>, speakers: u32) -> Transcript {
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: String::new(),
            audio: AudioInfo {
                file: String::new(),
                duration_sec: 0.0,
                sample_rate: 16_000,
            },
            engine: EngineInfo {
                vad: String::new(),
                asr: String::new(),
                embedding: String::new(),
            },
            language: SpeechLanguage::Ru,
            speakers: (1..=speakers)
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
            turns,
        }
    }

    fn truth() -> Truth {
        Truth::from_json(
            r#"{"speakers": 2, "turns": [
                {"speaker": 1, "start": 0.0, "end": 2.0, "text": "раз два"},
                {"speaker": 2, "start": 2.0, "end": 4.0, "text": "три"}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn perfect_transcript_scores_zero_everywhere() {
        let truth = truth();
        let t = transcript(
            vec![turn(1, 0.0, 2.0, "Раз, два."), turn(2, 2.0, 4.0, "Три.")],
            2,
        );
        let s = score_record(&RecordRun {
            name: "a",
            truth: &truth,
            transcript: &t,
            audio_sec: 4.0,
            processing_sec: 1.0,
            samples: None,
        })
        .unwrap();
        assert_eq!(s.der.rate(), Some(0.0));
        assert_eq!(s.wer.unwrap().rate(), Some(0.0));
        assert_eq!(s.cpwer.unwrap().rate(), Some(0.0));
        assert_eq!(s.rtf(), Some(0.25));
        assert_eq!((s.truth_speakers, s.found_speakers), (2, 2));
        assert_eq!(s.overlap.found_sec, 0.0);
    }

    #[test]
    fn overlap_marked_on_a_turn_is_scored_against_the_reference() {
        let truth = Truth::from_json(
            r#"{"speakers": 2, "turns": [
                {"speaker": 1, "start": 0.0, "end": 3.0, "text": "раз два"},
                {"speaker": 2, "start": 2.0, "end": 4.0, "text": "три"}
            ]}"#,
        )
        .unwrap();
        let mut marked = turn(1, 2.0, 3.0, "два");
        marked.flags = vec![nixyon_core::OVERLAP_FLAG.into()];
        let t = transcript(
            vec![turn(1, 0.0, 2.0, "раз"), marked, turn(2, 3.0, 4.0, "три")],
            2,
        );
        let s = score_record(&RecordRun {
            name: "a",
            truth: &truth,
            transcript: &t,
            audio_sec: 4.0,
            processing_sec: 1.0,
            samples: None,
        })
        .unwrap();
        assert_eq!(
            (s.overlap.precision(), s.overlap.recall()),
            (Some(1.0), Some(1.0))
        );
    }

    #[test]
    fn one_speaker_for_all_hurts_der_and_cpwer_but_not_wer() {
        let truth = truth();
        let t = transcript(vec![turn(1, 0.0, 4.0, "раз два три")], 1);
        let s = score_record(&RecordRun {
            name: "a",
            truth: &truth,
            transcript: &t,
            audio_sec: 4.0,
            processing_sec: 1.0,
            samples: None,
        })
        .unwrap();
        assert_eq!(s.wer.unwrap().errors, 0);
        assert_eq!(
            s.cpwer.unwrap(),
            ErrorCount {
                errors: 2,
                reference_len: 3
            }
        );
        assert!(s.der.confusion > 0.0);
    }

    #[test]
    fn names_from_samples_are_scored_against_reference_voices() {
        let truth = Truth::from_json(
            r#"{"speakers": 2, "turns": [
                {"speaker": 1, "start": 0.0, "end": 2.0, "text": "раз", "voice": "anna"},
                {"speaker": 2, "start": 2.0, "end": 4.0, "text": "два", "voice": "boris"}
            ]}"#,
        )
        .unwrap();
        let mut t = transcript(vec![turn(1, 0.0, 2.0, "раз"), turn(2, 2.0, 4.0, "два")], 2);
        t.speakers[0].name = Some("anna".into());
        t.speakers[1].name = Some("gost".into());
        let samples = ["anna".to_string(), "boris".to_string(), "gost".to_string()];
        let s = score_record(&RecordRun {
            name: "a",
            truth: &truth,
            transcript: &t,
            audio_sec: 4.0,
            processing_sec: 1.0,
            samples: Some(&samples),
        })
        .unwrap();
        let n = s.naming.unwrap();
        assert_eq!((n.expected, n.correct, n.wrong, n.missed()), (2, 1, 1, 1));
    }
}
