use std::path::Path;

use crate::{CancelFlag, EngineError, VoiceSpan};

/// Длина блока, которым запись подаётся в нейросегментацию по умолчанию: она берёт
/// звук целиком массивом, а час записи весит около 230 МБ.
pub const NEURAL_BLOCK_SEC: f64 = 60.0;

/// Нейросегментация: кто и когда говорит, включая наложения голосов. Отдельно от
/// `SpeechEngine`, потому что её умеет не каждый движок.
pub trait VoiceSegmenter {
    /// `speech`: куски VAD (начало и конец в секундах), отрезки обрезаются по ним.
    fn voice_spans(
        &self,
        wav_16k: &Path,
        speech: &[(f64, f64)],
        block_sec: f64,
        cancel: &CancelFlag,
    ) -> Result<Vec<VoiceSpan>, EngineError>;
}
