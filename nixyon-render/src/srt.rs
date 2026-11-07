use nixyon_core::{Transcript, Word};

use crate::clock::srt_timestamp;
use crate::labels::Labels;
use crate::names::speaker_name;
use crate::spread::spread_words;
use crate::txt::one_line;

pub const MAX_LINE_CHARS: usize = 42;
pub const MAX_LINES: usize = 2;
pub const MAX_CUE_SEC: f64 = 6.0;

struct Cue {
    start: f64,
    end: f64,
    lines: Vec<String>,
    words: usize,
}

impl Cue {
    fn open(start: f64, prefix: String) -> Self {
        Self {
            start,
            end: start,
            lines: vec![prefix],
            words: 0,
        }
    }

    fn last_len(&self) -> usize {
        self.lines.last().map_or(0, |l| l.chars().count())
    }

    // Слово встаёт в текущую строку, если в ней остаётся место до 42 знаков, иначе в
    // новую, пока строк меньше двух. Первое слово титра встаёт всегда; слово длиннее
    // строки занимает строку целиком.
    fn fits(&self, word: &str) -> bool {
        self.words == 0
            || self.last_len() + 1 + word.chars().count() <= MAX_LINE_CHARS
            || self.lines.len() < MAX_LINES
    }

    fn push(&mut self, word: &Word) {
        let text = one_line(&word.text);
        let len = text.chars().count();
        let last_len = self.last_len();
        let after_prefix = self.words == 0;
        let same_line = if after_prefix {
            last_len == 0 || last_len + len <= MAX_LINE_CHARS
        } else {
            last_len + 1 + len <= MAX_LINE_CHARS
        };
        if same_line {
            let last = self.lines.last_mut().expect("в титре всегда есть строка");
            if !after_prefix {
                last.push(' ');
            }
            last.push_str(&text);
        } else {
            if let Some(last) = self.lines.last_mut() {
                last.truncate(last.trim_end().len());
            }
            self.lines.push(text);
        }
        self.words += 1;
        self.end = word.end;
    }
}

/// Субтитры SRT: титр не длиннее 6 с, до двух строк по 42 знака, имя говорящего в
/// первом титре каждого хода. Время слов берётся из движка или раскладывается по буквам.
pub fn render_srt(transcript: &Transcript, labels: &Labels) -> String {
    let mut cues: Vec<Cue> = Vec::new();
    for turn in &transcript.turns {
        let words = spread_words(turn);
        let Some(first) = words.first() else {
            continue;
        };
        let name = one_line(&speaker_name(transcript, turn.speaker, labels));
        let prefix = match labels.emotion(turn.emotion) {
            Some(emotion) => format!("{name} ({}): ", one_line(emotion)),
            None => format!("{name}: "),
        };
        let mut cue = Cue::open(first.start, prefix);
        for word in &words {
            let too_long = word.end - cue.start > MAX_CUE_SEC;
            if cue.words > 0 && (too_long || !cue.fits(&one_line(&word.text))) {
                cues.push(cue);
                cue = Cue::open(word.start, String::new());
            }
            cue.push(word);
        }
        cues.push(cue);
    }
    let mut out = String::new();
    for (i, cue) in cues.iter().enumerate() {
        // Титр нулевой длины плееры пропускают: держим хотя бы миллисекунду.
        let end = cue.end.max(cue.start + 0.001);
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            srt_timestamp(cue.start),
            srt_timestamp(end),
            cue.lines.join("\n")
        ));
    }
    out
}
