use nixyon_core::{
    AudioInfo, EngineInfo, SCHEMA_VERSION, SegmentResult, Speaker, Transcript, Turn, Word,
};

use crate::TranscriptMeta;

/// Расшифровка из ходов (спикер, начало, конец, текст); спикеры берутся из ходов.
pub fn transcript(turns: &[(u32, f64, f64, &str)]) -> Transcript {
    let mut ids: Vec<u32> = turns.iter().map(|t| t.0).collect();
    ids.sort_unstable();
    ids.dedup();
    Transcript {
        schema_version: SCHEMA_VERSION,
        created_at: "2026-10-28T00:00:00Z".into(),
        audio: AudioInfo {
            file: "a.wav".into(),
            duration_sec: turns.iter().map(|t| t.2).fold(0.0, f64::max),
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
            .map(|(&(speaker, start, end, text), id)| Turn {
                id,
                speaker,
                start,
                end,
                text: text.into(),
                words: None,
                flags: vec![],
            })
            .collect(),
    }
}

pub fn word(text: &str, start: f64, end: f64) -> Word {
    Word {
        text: text.into(),
        start,
        end,
    }
}

pub fn segment(start: f64, end: f64, text: &str, embedding: Vec<f32>) -> SegmentResult {
    SegmentResult {
        start,
        end,
        text: text.into(),
        tokens: Vec::new(),
        token_times: Vec::new(),
        embedding,
        windows: Vec::new(),
    }
}

/// Эмбеддинг «голоса» `id`: свой базисный вектор плюс небольшая примесь `noise`.
pub fn voice(id: usize, noise: f32) -> Vec<f32> {
    let mut v = vec![noise; 8];
    v[id % 8] = 1.0;
    v
}

pub fn meta() -> TranscriptMeta {
    TranscriptMeta {
        created_at: "2026-10-28T00:00:00Z".into(),
        audio: AudioInfo {
            file: "a.wav".into(),
            duration_sec: 10.0,
            sample_rate: 16_000,
        },
        engine: EngineInfo {
            vad: "vad".into(),
            asr: "asr".into(),
            embedding: "emb".into(),
        },
    }
}
