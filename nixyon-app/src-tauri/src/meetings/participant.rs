use nixyon_core::Transcript;
use serde::{Deserialize, Serialize};

/// Участник встречи в индексе: номер спикера и имя, если его дали. Подпись «Спикер N»
/// подставляет интерфейс на своём языке.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Participant {
    pub id: u32,
    pub name: Option<String>,
}

/// Участники в том же порядке, что на экране расшифровки: дольше говорил, выше; при
/// равном времени меньший номер выше.
pub fn participants(transcript: &Transcript) -> Vec<Participant> {
    let mut speakers: Vec<_> = transcript.speakers.iter().collect();
    speakers.sort_by(|a, b| {
        b.talk_time_sec
            .total_cmp(&a.talk_time_sec)
            .then(a.id.cmp(&b.id))
    });
    speakers
        .into_iter()
        .map(|s| Participant {
            id: s.id,
            name: s.name.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use nixyon_core::Speaker;

    use super::*;

    fn speaker(id: u32, talk: f64, name: Option<&str>) -> Speaker {
        Speaker {
            id,
            name: name.map(str::to_owned),
            talk_time_sec: talk,
            turns: 1,
            words: 1,
            age: None,
            gender: None,
        }
    }

    #[test]
    fn participants_go_by_talk_time_then_by_number_and_keep_names() {
        let mut t = super::super::test_transcript("2026-11-07T10:00:00Z");
        t.speakers = vec![
            speaker(1, 5.0, None),
            speaker(2, 9.0, Some("Анна")),
            speaker(3, 5.0, Some("Борис")),
        ];
        let got = participants(&t);
        let order: Vec<_> = got.iter().map(|p| (p.id, p.name.as_deref())).collect();
        assert_eq!(
            order,
            vec![(2, Some("Анна")), (1, None), (3, Some("Борис"))]
        );
    }
}
