use nixyon_core::{Transcript, VoiceSample};
use nixyon_pipeline::SpeakerCenter;

use crate::Truth;

/// Библиотека голосов по прошлой встрече, как её собрал бы пользователь: каждому
/// найденному спикеру имя того голоса эталона, с которым он дольше всего совпадает по
/// времени, и его центр запоминается под этим именем. Спикер без совпадений пропускается.
pub fn library_from_meeting(
    transcript: &Transcript,
    centers: &[SpeakerCenter],
    truth: &Truth,
) -> Vec<VoiceSample> {
    centers
        .iter()
        .filter_map(|(speaker, center)| {
            Some(VoiceSample {
                name: dominant_voice(transcript, *speaker, truth)?,
                embedding: center.clone(),
            })
        })
        .collect()
}

// Голос эталона с наибольшим общим временем с ходами спикера; при равенстве первый по имени.
fn dominant_voice(transcript: &Transcript, speaker: u32, truth: &Truth) -> Option<String> {
    let mut totals: Vec<(&str, f64)> = Vec::new();
    for t in &truth.turns {
        let Some(voice) = t.voice.as_deref() else {
            continue;
        };
        let overlap: f64 = transcript
            .turns
            .iter()
            .filter(|h| h.speaker == speaker)
            .map(|h| (h.end.min(t.end) - h.start.max(t.start)).max(0.0))
            .sum();
        match totals.iter_mut().find(|(v, _)| *v == voice) {
            Some(entry) => entry.1 += overlap,
            None => totals.push((voice, overlap)),
        }
    }
    totals.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
    totals
        .first()
        .filter(|(_, overlap)| *overlap > 0.0)
        .map(|(voice, _)| (*voice).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION, Turn};

    fn hypothesis(turns: &[(u32, f64, f64)]) -> Transcript {
        Transcript {
            schema_version: SCHEMA_VERSION,
            created_at: String::new(),
            audio: AudioInfo {
                file: "a.wav".into(),
                duration_sec: 10.0,
                sample_rate: 16_000,
            },
            engine: EngineInfo {
                vad: String::new(),
                asr: String::new(),
                embedding: String::new(),
            },
            speakers: Vec::new(),
            turns: turns
                .iter()
                .zip(1..)
                .map(|(&(speaker, start, end), id)| Turn {
                    id,
                    speaker,
                    start,
                    end,
                    text: "а".into(),
                    words: None,
                    flags: Vec::new(),
                    emotion: None,
                })
                .collect(),
        }
    }

    fn truth() -> Truth {
        Truth::from_json(
            r#"{"speakers": 2, "turns": [
                {"speaker": 1, "start": 0.0, "end": 4.0, "voice": "anna"},
                {"speaker": 2, "start": 4.0, "end": 6.0, "voice": "boris"}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn each_speaker_center_is_named_after_the_voice_it_overlaps_most() {
        let h = hypothesis(&[(1, 0.0, 3.0), (2, 3.0, 6.0)]);
        let centers = vec![(1, vec![1.0, 0.0]), (2, vec![0.0, 1.0])];
        let library = library_from_meeting(&h, &centers, &truth());
        let names: Vec<&str> = library.iter().map(|s| s.name.as_str()).collect();
        // Спикер 2: 1 с у anna и 2 с у boris.
        assert_eq!(names, ["anna", "boris"]);
        assert_eq!(library[1].embedding, vec![0.0, 1.0]);
    }

    #[test]
    fn speaker_without_overlap_is_skipped() {
        let h = hypothesis(&[(1, 0.0, 3.0), (2, 7.0, 9.0)]);
        let centers = vec![(1, vec![1.0]), (2, vec![1.0])];
        let library = library_from_meeting(&h, &centers, &truth());
        assert_eq!(library.len(), 1);
    }
}
