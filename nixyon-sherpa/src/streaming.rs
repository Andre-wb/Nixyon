//! Потоковое распознавание для живого текста во время записи: текст без спикеров, по
//! фразам. Спикеры и финальный текст даёт расшифровка после остановки.

mod live_text;
mod stream_model;

pub use live_text::LiveText;
pub use stream_model::StreamModel;
