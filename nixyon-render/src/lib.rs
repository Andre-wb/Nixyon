//! Рендереры расшифровки: TXT, Markdown, SRT, JSON и протокол DOCX. Подписи приходят
//! структурой `Labels` на языке интерфейса; дата записи приходит готовой строкой, её
//! форматирует интерфейс в поясе устройства.

mod clock;
mod docx;
mod json;
mod labels;
mod markdown;
mod names;
mod protocol;
mod spread;
mod srt;
mod txt;

pub use clock::{clock, srt_timestamp};
pub use docx::{document_xml, render_docx};
pub use json::render_json;
pub use labels::Labels;
pub use markdown::render_markdown;
pub use protocol::{Block, protocol_blocks};
pub use spread::spread_words;
pub use srt::{MAX_CUE_SEC, MAX_LINE_CHARS, MAX_LINES, render_srt};
pub use txt::render_txt;
