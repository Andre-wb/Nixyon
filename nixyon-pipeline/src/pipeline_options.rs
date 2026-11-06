use nixyon_cluster::SAME_SPEAKER_CENTROID_COSINE;
use nixyon_core::{EngineOptions, SpeechEdges, WindowSpec};

use crate::{LONG_SEGMENT_SEC, MAX_AUTO_SPEAKERS, MIN_SPEAKER_SEC, SAMPLE_THRESHOLD, VoiceMatch};

/// Настройки второй фазы. Значения по умолчанию: лучшая конфигурация по замеру,
/// её берёт приложение; замер (`eval`) меняет их, чтобы сравнить варианты.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineOptions {
    /// Куски от этой длины идут в кластеризацию, короткие потом присоединяются
    /// к ближайшему центру.
    pub long_segment_sec: f64,
    /// Верхняя граница числа спикеров, когда пользователь его не знает.
    pub max_auto_speakers: usize,
    /// Когда число спикеров неизвестно, группы с центрами похожее этого косинуса
    /// считаются одним человеком.
    pub same_speaker_cosine: f64,
    /// Когда число спикеров неизвестно, группа с меньшим временем речи не спикер: её
    /// куски уходят к ближайшему спикеру. 0: правило выключено.
    pub min_speaker_sec: f64,
    /// Пересегментация Витерби: штраф за смену говорящего в единицах косинуса.
    /// None: выключена.
    pub viterbi_switch_penalty: Option<f64>,
    /// Разрез куска по словам там, где окна внутри него принадлежат разным говорящим.
    /// Без Витерби окна размечаются ближайшим центром, без сглаживания.
    pub word_cut: bool,
    /// Окна для эмбеддингов голоса, которые просят пересегментация и разрез по словам.
    pub window: WindowSpec,
    /// Говорящие по отрезкам нейросегментации, с пометкой перекрытий. Тогда окна
    /// не нужны: Витерби и разрез по словам не работают.
    pub neural_segmentation: bool,
    /// Наименьший косинус между центром голоса спикера и образцом, при котором спикер
    /// получает имя из образца.
    pub sample_threshold: f64,
    /// Как сравнивать спикера с человеком, у которого несколько эмбеддингов.
    pub voice_match: VoiceMatch,
    /// Наибольшая длина куска речи, секунды. None: как в приложении (20 с).
    pub max_speech_sec: Option<f32>,
    /// Не распознавать куски, где вместо речи звучит музыка или пение.
    pub drop_music: bool,
    /// Распознавать соседние куски с короткой паузой вместе, как приложение на Android.
    pub asr_groups: bool,
}

impl PipelineOptions {
    /// Что нужно попросить у движка под эти настройки: окна только тому, кто их использует.
    pub fn engine_options(&self) -> EngineOptions {
        EngineOptions {
            windows: self
                .window_penalty()
                .filter(|_| !self.neural_segmentation)
                .map(|_| self.window),
            hotwords: Vec::new(),
            edges: if self.neural_segmentation {
                SpeechEdges::Padded
            } else {
                SpeechEdges::Tight
            },
            max_speech_sec: self.max_speech_sec,
            drop_music: self.drop_music,
            asr_groups: self.asr_groups,
        }
    }

    /// Штраф Витерби для разметки окон: заданный, а при одном разрезе по словам 0
    /// (каждое окно к ближайшему центру). None: окна не размечаются.
    pub(crate) fn window_penalty(&self) -> Option<f64> {
        self.viterbi_switch_penalty.or(self.word_cut.then_some(0.0))
    }
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            long_segment_sec: LONG_SEGMENT_SEC,
            max_auto_speakers: MAX_AUTO_SPEAKERS,
            same_speaker_cosine: SAME_SPEAKER_CENTROID_COSINE,
            // Шум и обрывки речи в шумной записи иначе занимают места спикеров, и два
            // похожих голоса сливаются: по замеру путаница 9,0 % стала 1,8 %.
            min_speaker_sec: MIN_SPEAKER_SEC,
            viterbi_switch_penalty: None,
            // Разрез по словам: при выключенной нейросегментации на смене голоса без паузы
            // и на наложениях DER заметно ниже, на записях с паузами без изменений. Витерби
            // выключен: поверх разреза он ничего не добавил, а штраф от 0,3 ухудшил DER.
            word_cut: true,
            window: WindowSpec::DEFAULT,
            // Нейросегментация включена: по замеру DER и cpWER ниже, чем у разреза по словам,
            // особенно на смене голоса без паузы; окна при ней не считаются.
            neural_segmentation: true,
            sample_threshold: SAMPLE_THRESHOLD,
            // По замеру с образцами и центрами двух встреч оба способа дают одно и то же;
            // наибольший косинус у человека с одним эмбеддингом совпадает с прежним сравнением.
            voice_match: VoiceMatch::Max,
            max_speech_sec: None,
            drop_music: false,
            asr_groups: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neural_segmentation_asks_the_engine_for_no_windows() {
        assert_eq!(PipelineOptions::default().engine_options().windows, None);
        let vad_only = PipelineOptions {
            neural_segmentation: false,
            ..PipelineOptions::default()
        };
        assert!(vad_only.engine_options().windows.is_some());
    }

    #[test]
    fn only_speaker_split_by_vad_asks_for_tight_speech_edges() {
        assert_eq!(
            PipelineOptions::default().engine_options().edges,
            SpeechEdges::Padded
        );
        let vad_only = PipelineOptions {
            neural_segmentation: false,
            ..PipelineOptions::default()
        };
        assert_eq!(vad_only.engine_options().edges, SpeechEdges::Tight);
    }
}
