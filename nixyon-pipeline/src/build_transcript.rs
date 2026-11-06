use nixyon_core::{
    CancelFlag, SCHEMA_VERSION, SegmentResult, Speaker, Transcript, Turn, VoiceSample, VoiceSpan,
};

use crate::neural_turns::neural_turns;
use crate::sample_names::{SpeakerCenter, sample_names};
use crate::sentence_fix::fix_split_sentences;
use crate::speaker_labels::label_segments;
use crate::speaker_names::{apply_names, intro_names};
use crate::turns::glue_turns;
use crate::word_cut::cut_by_words;
use crate::words::tokens_to_words;
use crate::{
    PipelineError, PipelineOptions, SpeakerCount, TranscriptMeta, recompute_speaker_stats,
};

/// Вторая фаза: из кусков речи от движка собирает расшифровку со спикерами.
/// Куски без текста отбрасываются; ходы одного спикера с короткой паузой склеиваются.
/// `spans`: отрезки нейросегментации, нужны только при `options.neural_segmentation`.
/// `samples`: образцы голоса; спикер, похожий на образец, получает его имя.
pub fn build_transcript(
    segments: Vec<SegmentResult>,
    spans: &[VoiceSpan],
    samples: &[VoiceSample],
    count: SpeakerCount,
    options: &PipelineOptions,
    meta: TranscriptMeta,
    cancel: &CancelFlag,
) -> Result<Transcript, PipelineError> {
    build_transcript_with_centers(segments, spans, samples, count, options, meta, cancel)
        .map(|(transcript, _)| transcript)
}

/// То же, что `build_transcript`, и вдобавок центры голосов спикеров расшифровки (по
/// возрастанию номера): по ним голос запоминают в библиотеку.
pub fn build_transcript_with_centers(
    mut segments: Vec<SegmentResult>,
    spans: &[VoiceSpan],
    samples: &[VoiceSample],
    count: SpeakerCount,
    options: &PipelineOptions,
    meta: TranscriptMeta,
    cancel: &CancelFlag,
) -> Result<(Transcript, Vec<SpeakerCenter>), PipelineError> {
    check(cancel)?;
    segments.retain(|s| !s.text.trim().is_empty());
    if segments.is_empty() {
        return Err(PipelineError::NoSpeech);
    }
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));

    let (turns, centers) = if options.neural_segmentation {
        neural_turns(&segments, spans, count, options)
    } else {
        vad_turns(segments, count, options)
    };
    check(cancel)?;
    let mut turns = glue_turns(fix_split_sentences(glue_turns(turns)));
    for (turn, id) in turns.iter_mut().zip(1..) {
        turn.id = id;
    }

    let names = sample_names(
        &centers,
        samples,
        options.sample_threshold,
        options.voice_match,
    );
    let mut ids: Vec<u32> = turns.iter().map(|t| t.speaker).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut transcript = Transcript {
        schema_version: SCHEMA_VERSION,
        created_at: meta.created_at,
        audio: meta.audio,
        engine: meta.engine,
        speakers: ids
            .into_iter()
            .map(|id| Speaker {
                id,
                name: names
                    .iter()
                    .find(|(speaker, _)| *speaker == id)
                    .map(|(_, name)| name.clone()),
                talk_time_sec: 0.0,
                turns: 0,
                words: 0,
                age: None,
                gender: None,
            })
            .collect(),
        turns,
    };
    let hints = intro_names(&transcript.turns);
    apply_names(&mut transcript.speakers, hints);
    recompute_speaker_stats(&mut transcript);
    let mut centers: Vec<SpeakerCenter> = centers
        .into_iter()
        .filter(|(id, _)| transcript.speaker(*id).is_some())
        .collect();
    centers.sort_by_key(|(id, _)| *id);
    Ok((transcript, centers))
}

// Ходы по кускам VAD: метка куска из кластеризации, при разрезе по словам кусок
// режется там, где окна внутри него разных говорящих. Вместе с ходами центры голосов.
fn vad_turns(
    segments: Vec<SegmentResult>,
    count: SpeakerCount,
    options: &PipelineOptions,
) -> (Vec<Turn>, Vec<SpeakerCenter>) {
    let labels = label_segments(&segments, count, options);
    let centers = labels.centers;

    let turns = segments
        .into_iter()
        .zip(labels.segments)
        .zip(labels.windows)
        .flat_map(|((s, speaker), windows)| {
            let words = tokens_to_words(&s.tokens, &s.token_times, s.start, s.end);
            let text = s.text.trim().to_string();
            // Без разреза окна только уточняют метку куска, кусок остаётся одним ходом.
            let windows = if options.word_cut {
                windows.as_slice()
            } else {
                &[]
            };
            cut_by_words(s.start, s.end, text, words, speaker, windows)
        })
        .collect();
    (turns, centers)
}

fn check(cancel: &CancelFlag) -> Result<(), PipelineError> {
    if cancel.is_cancelled() {
        Err(PipelineError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nixyon_core::WindowEmbedding;

    use super::*;
    use crate::test_support::{meta, segment, voice};

    fn letters(text: &str) -> (Vec<String>, Vec<f32>) {
        let tokens: Vec<String> = text.chars().map(String::from).collect();
        let times = (0..tokens.len()).map(|i| i as f32 * 0.04).collect();
        (tokens, times)
    }

    fn with_tokens(mut s: SegmentResult) -> SegmentResult {
        let (tokens, times) = letters(&s.text);
        s.tokens = tokens;
        s.token_times = times;
        s
    }

    #[test]
    fn dialog_becomes_turns_with_speakers_words_and_stats() {
        let segs = vec![
            with_tokens(segment(0.0, 2.0, "добрый день", voice(1, 0.0))),
            with_tokens(segment(2.3, 4.0, "привет", voice(2, 0.0))),
            with_tokens(segment(4.2, 6.0, "начнём", voice(1, 0.1))),
            with_tokens(segment(6.3, 6.9, "да", voice(1, 0.1))),
        ];
        let t = build_transcript(
            segs,
            &[],
            &[],
            SpeakerCount::Auto,
            &PipelineOptions::default(),
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        let shape: Vec<(u32, u32, &str)> = t
            .turns
            .iter()
            .map(|x| (x.id, x.speaker, x.text.as_str()))
            .collect();
        assert_eq!(
            shape,
            [(1, 1, "добрый день"), (2, 2, "привет"), (3, 1, "начнём да")]
        );
        let words: Vec<&str> = t.turns[0]
            .words
            .as_ref()
            .unwrap()
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert_eq!(words, ["добрый", "день"]);
        assert_eq!(t.speakers.len(), 2);
        assert_eq!(t.speaker(1).unwrap().turns, 2);
        assert_eq!(t.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn word_cut_splits_a_segment_where_two_voices_meet_without_a_pause() {
        let mut mixed = with_tokens(segment(2.0, 5.0, "да нет", voice(1, 0.0)));
        // «да» звучит в начале, «нет» в конце куска.
        mixed.token_times = vec![0.0, 0.2, 0.4, 2.0, 2.2, 2.4];
        mixed.windows = vec![
            WindowEmbedding {
                start: 2.0,
                end: 3.5,
                embedding: voice(1, 0.0),
            },
            WindowEmbedding {
                start: 3.5,
                end: 5.0,
                embedding: voice(2, 0.0),
            },
        ];
        let segs = vec![
            with_tokens(segment(0.0, 1.8, "раз", voice(1, 0.0))),
            mixed,
            with_tokens(segment(5.5, 7.5, "два", voice(2, 0.0))),
        ];
        let whole = PipelineOptions {
            word_cut: false,
            neural_segmentation: false,
            ..PipelineOptions::default()
        };
        let cut = PipelineOptions {
            neural_segmentation: false,
            ..PipelineOptions::default()
        };
        let plain = build_transcript(
            segs.clone(),
            &[],
            &[],
            SpeakerCount::Known(2),
            &whole,
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        assert_eq!(plain.turns.len(), 2);
        let t = build_transcript(
            segs,
            &[],
            &[],
            SpeakerCount::Known(2),
            &cut,
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        let shape: Vec<(u32, &str)> = t
            .turns
            .iter()
            .map(|x| (x.speaker, x.text.as_str()))
            .collect();
        assert_eq!(shape, [(1, "раз да"), (2, "нет два")]);
    }

    #[test]
    fn neural_segmentation_marks_the_overlap_as_its_own_turn_that_is_not_glued() {
        use nixyon_core::{OVERLAP_FLAG, VoiceSpan};

        let span = |start: f64, end: f64, embedding: Vec<f32>| VoiceSpan {
            start,
            end,
            embedding,
        };
        let mut seg = segment(0.0, 6.0, "а б в г", voice(1, 0.0));
        seg.tokens = ["▁а", "▁б", "▁в", "▁г"].map(String::from).to_vec();
        seg.token_times = vec![0.5, 2.0, 3.2, 4.5];
        let spans = vec![
            span(0.0, 4.0, voice(1, 0.0)),
            span(3.0, 6.0, voice(2, 0.0)),
            span(7.0, 9.0, voice(2, 0.1)),
        ];
        let neural = PipelineOptions::default();
        let vad_only = PipelineOptions {
            neural_segmentation: false,
            ..PipelineOptions::default()
        };
        let t = build_transcript(
            vec![seg.clone()],
            &spans,
            &[],
            SpeakerCount::Known(2),
            &neural,
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        let shape: Vec<(u32, &str, bool)> = t
            .turns
            .iter()
            .map(|x| {
                (
                    x.speaker,
                    x.text.as_str(),
                    x.flags.iter().any(|f| f == OVERLAP_FLAG),
                )
            })
            .collect();
        assert_eq!(shape, [(1, "а б", false), (1, "в", true), (2, "г", false)]);
        assert_eq!(t.speakers.len(), 2);

        // Без флага отрезки не используются, кусок остаётся одним ходом.
        let plain = build_transcript(
            vec![seg],
            &spans,
            &[],
            SpeakerCount::Known(2),
            &vad_only,
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        assert_eq!(plain.turns.len(), 1);
    }

    #[test]
    fn speakers_similar_to_samples_get_their_names() {
        use nixyon_core::VoiceSample;

        let segs = vec![
            segment(0.0, 2.0, "добрый день", voice(1, 0.0)),
            segment(2.3, 4.0, "привет", voice(2, 0.0)),
            segment(4.2, 6.0, "начнём", voice(1, 0.1)),
        ];
        let samples = [
            VoiceSample {
                name: "Борис".into(),
                embedding: voice(2, 0.05),
            },
            VoiceSample {
                name: "Гость".into(),
                embedding: voice(6, 0.0),
            },
        ];
        let t = build_transcript(
            segs,
            &[],
            &samples,
            SpeakerCount::Known(2),
            &PipelineOptions::default(),
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        let names: Vec<Option<&str>> = t.speakers.iter().map(|s| s.name.as_deref()).collect();
        assert_eq!(names, [None, Some("Борис")]);
    }

    #[test]
    fn segments_are_sorted_by_time_and_empty_text_is_dropped() {
        let segs = vec![
            segment(5.0, 7.0, "второй", voice(1, 0.0)),
            segment(2.0, 4.0, "   ", voice(1, 0.0)),
            segment(0.0, 2.0, "первый", voice(1, 0.0)),
        ];
        let t = build_transcript(
            segs,
            &[],
            &[],
            SpeakerCount::Known(1),
            &PipelineOptions::default(),
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        let texts: Vec<&str> = t.turns.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, ["первый", "второй"]);
    }

    #[test]
    fn no_text_at_all_is_no_speech() {
        let segs = vec![segment(0.0, 2.0, "", voice(1, 0.0))];
        assert_eq!(
            build_transcript(
                segs,
                &[],
                &[],
                SpeakerCount::Auto,
                &PipelineOptions::default(),
                meta(),
                &CancelFlag::new()
            ),
            Err(PipelineError::NoSpeech)
        );
        assert_eq!(
            build_transcript(
                vec![],
                &[],
                &[],
                SpeakerCount::Auto,
                &PipelineOptions::default(),
                meta(),
                &CancelFlag::new()
            ),
            Err(PipelineError::NoSpeech)
        );
    }

    #[test]
    fn cancelled_flag_stops_processing() {
        let cancel = CancelFlag::new();
        cancel.cancel();
        let segs = vec![segment(0.0, 2.0, "а", voice(1, 0.0))];
        assert_eq!(
            build_transcript(
                segs,
                &[],
                &[],
                SpeakerCount::Auto,
                &PipelineOptions::default(),
                meta(),
                &cancel
            ),
            Err(PipelineError::Cancelled)
        );
    }

    #[test]
    fn result_survives_json_round_trip() {
        let segs = vec![with_tokens(segment(0.0, 2.0, "да нет", voice(1, 0.0)))];
        let t = build_transcript(
            segs,
            &[],
            &[],
            SpeakerCount::Auto,
            &PipelineOptions::default(),
            meta(),
            &CancelFlag::new(),
        )
        .unwrap();
        assert_eq!(
            nixyon_core::from_json(&nixyon_core::to_json(&t).unwrap()).unwrap(),
            t
        );
    }
}
