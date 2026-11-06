use std::fmt;

use nixyon_core::Transcript;

/// Наименьшее время речи крупного спикера, секунды. Меньше обычно обрывки чужого
/// голоса или шум, а не отдельный человек.
pub const LARGE_SPEAKER_SEC: f64 = 5.0;

/// Итог прогона записи без разметки: только то, что видно по самой расшифровке.
#[derive(Debug, Clone, PartialEq)]
pub struct UnscoredRecord {
    pub name: String,
    pub audio_sec: f64,
    pub processing_sec: f64,
    pub speakers: usize,
    pub large_speakers: usize,
}

impl UnscoredRecord {
    pub fn new(name: &str, transcript: &Transcript, audio_sec: f64, processing_sec: f64) -> Self {
        Self {
            name: name.to_owned(),
            audio_sec,
            processing_sec,
            speakers: transcript.speakers.len(),
            large_speakers: transcript
                .speakers
                .iter()
                .filter(|s| s.talk_time_sec >= LARGE_SPEAKER_SEC)
                .count(),
        }
    }

    /// Время обработки к длительности записи; меньше 1: быстрее реального времени.
    pub fn rtf(&self) -> Option<f64> {
        (self.audio_sec > 0.0).then(|| self.processing_sec / self.audio_sec)
    }
}

impl fmt::Display for UnscoredRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rtf = self
            .rtf()
            .map_or_else(|| "нет".to_owned(), |r| format!("{r:.3}"));
        write!(
            f,
            "{}: звук {:.1} с, обработка {:.1} с, RTF {rtf}, спикеров {} (крупных от {} с: {})",
            self.name,
            self.audio_sec,
            self.processing_sec,
            self.speakers,
            LARGE_SPEAKER_SEC,
            self.large_speakers
        )
    }
}

#[cfg(test)]
mod tests {
    use nixyon_core::{AudioInfo, EngineInfo, SCHEMA_VERSION, Speaker};

    use super::*;

    fn transcript(talk_times: &[f64]) -> Transcript {
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
            speakers: talk_times
                .iter()
                .zip(1..)
                .map(|(&talk_time_sec, id)| Speaker {
                    id,
                    name: None,
                    talk_time_sec,
                    turns: 0,
                    words: 0,
                    age: None,
                    gender: None,
                })
                .collect(),
            turns: Vec::new(),
        }
    }

    #[test]
    fn large_speakers_talk_at_least_five_seconds() {
        let r = UnscoredRecord::new("live", &transcript(&[120.0, 5.0, 4.9, 0.0]), 200.0, 50.0);
        assert_eq!((r.speakers, r.large_speakers), (4, 2));
        assert_eq!(r.rtf(), Some(0.25));
    }

    #[test]
    fn line_shows_rtf_and_both_speaker_counts() {
        let r = UnscoredRecord::new("live", &transcript(&[30.0, 1.0]), 100.0, 33.3);
        assert_eq!(
            r.to_string(),
            "live: звук 100.0 с, обработка 33.3 с, RTF 0.333, спикеров 2 (крупных от 5 с: 1)"
        );
    }

    #[test]
    fn empty_audio_has_no_rtf() {
        let r = UnscoredRecord::new("live", &transcript(&[]), 0.0, 1.0);
        assert_eq!(r.rtf(), None);
        assert!(r.to_string().contains("RTF нет"), "{r}");
    }
}
