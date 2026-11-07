//! Встречи для тестов поиска по архиву.

use nixyon_core::{Transcript, Turn};

use crate::meetings::MeetingId;
use crate::vault::FilesStore;

/// id встречи по номеру: чем больше номер, тем новее встреча.
pub fn meeting_id(n: u64) -> String {
    MeetingId::from_unix_secs(1_800_000_000 + n * 7200)
        .as_str()
        .to_owned()
}

pub fn transcript(n: u64, texts: &[String]) -> Transcript {
    let mut t = crate::meetings::test_transcript(&meeting_id(n));
    t.turns = texts
        .iter()
        .zip(1..)
        .map(|(text, id)| Turn {
            id,
            speaker: 1,
            start: f64::from(id) * 6.0,
            end: f64::from(id) * 6.0 + 5.0,
            text: text.clone(),
            words: None,
            flags: Vec::new(),
            emotion: None,
        })
        .collect();
    t.audio.duration_sec = t.turns.last().map_or(1.5, |turn| turn.end);
    t
}

/// Встреча в архиве с репликами `texts`; возвращает её id.
pub fn add(store: &FilesStore, n: u64, texts: &[&str]) -> String {
    let texts: Vec<String> = texts.iter().map(|&t| t.to_owned()).collect();
    crate::meetings::test_meeting(store, &transcript(n, &texts));
    meeting_id(n)
}

const WORDS: [&str; 24] = [
    "встреча",
    "проект",
    "завтра",
    "сроки",
    "бюджет",
    "отчёт",
    "ещё",
    "клиент",
    "договор",
    "задача",
    "решение",
    "вопрос",
    "команда",
    "релиз",
    "тестирование",
    "предложение",
    "да",
    "нет",
    "хорошо",
    "понятно",
    "сделаем",
    "обсудим",
    "пятница",
    "ёлка",
];

/// Встреча из `turns` реплик по 15 слов. Простой генератор: у каждой встречи и реплики
/// свой набор слов, одинаковый от запуска к запуску.
pub fn add_generated(store: &FilesStore, n: u64, turns: usize) {
    let mut seed = n.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let texts: Vec<String> = (0..turns)
        .map(|_| {
            (0..15)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    WORDS[usize::try_from(seed >> 33).unwrap_or(0) % WORDS.len()]
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    crate::meetings::test_meeting(store, &transcript(n, &texts));
}
