//! Протокол встречи для документов DOCX и PDF: то же содержимое, что в Markdown
//! (заголовок, сводка, участники по времени речи, ходы), но готовыми блоками, которые
//! остаётся только сверстать. PDF верстает Kotlin, поэтому блоки сериализуются.

mod participant_rows;

use nixyon_core::Transcript;
use serde::Serialize;

use crate::clock::clock;
use crate::labels::Labels;
use crate::names::speaker_name;
use crate::txt::one_line;

use participant_rows::participant_rows;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    Title {
        text: String,
    },
    Fact {
        label: String,
        value: String,
    },
    /// Первый столбец текстовый, остальные числовые и выравниваются вправо.
    Table {
        header: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Turn {
        speaker: String,
        time: String,
        text: String,
    },
}

pub fn protocol_blocks(
    transcript: &Transcript,
    labels: &Labels,
    recorded_at: Option<&str>,
) -> Vec<Block> {
    let mut blocks = vec![Block::Title {
        text: labels.title.clone(),
    }];
    if let Some(when) = recorded_at {
        blocks.push(Block::Fact {
            label: labels.recorded.clone(),
            value: one_line(when),
        });
    }
    blocks.push(Block::Fact {
        label: labels.duration.clone(),
        value: clock(transcript.audio.duration_sec),
    });
    blocks.push(Block::Fact {
        label: labels.participants.clone(),
        value: transcript.speakers.len().to_string(),
    });
    if !transcript.speakers.is_empty() {
        blocks.push(Block::Table {
            header: vec![
                labels.participant.clone(),
                labels.talk_time.clone(),
                labels.share.clone(),
            ],
            rows: participant_rows(transcript, labels),
        });
    }
    blocks.extend(transcript.turns.iter().map(|turn| Block::Turn {
        speaker: one_line(&speaker_name(transcript, turn.speaker, labels)),
        time: clock(turn.start),
        text: one_line(&turn.text),
    }));
    blocks
}
