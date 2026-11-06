use std::collections::BTreeMap;

use serde::Serialize;

use crate::{ClipDer, ClipResult, SubstitutionCount, TOP_SUBSTITUTIONS, top_substitutions};

/// Сколько худших клипов попадает в отчёт.
pub const WORST_CLIPS: usize = 50;

/// Границы длины клипа в секундах: до 3, от 3 до 5, от 5 до 8, от 8.
const DURATION_EDGES: [f64; 3] = [3.0, 5.0, 8.0];

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct ErrorTotal {
    pub errors: usize,
    pub reference: usize,
    pub rate: Option<f64>,
}

impl ErrorTotal {
    pub(crate) fn of(pairs: impl Iterator<Item = (usize, usize)>) -> Self {
        let (errors, reference) = pairs.fold((0, 0), |(e, r), (de, dr)| (e + de, r + dr));
        Self {
            errors,
            reference,
            rate: (reference > 0).then(|| errors as f64 / reference as f64),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupScore {
    pub label: String,
    pub clips: usize,
    pub audio_sec: f64,
    pub wer: ErrorTotal,
    pub cer: ErrorTotal,
    pub empty: usize,
}

impl GroupScore {
    fn of(label: String, clips: &[&ClipResult]) -> Self {
        Self {
            label,
            clips: clips.len(),
            audio_sec: clips.iter().map(|c| c.duration_sec).sum(),
            wer: ErrorTotal::of(clips.iter().map(|c| (c.word_errors, c.words))),
            cer: ErrorTotal::of(clips.iter().map(|c| (c.char_errors, c.chars))),
            empty: clips.iter().filter(|c| c.is_empty()).count(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QualitySummary {
    pub overall: GroupScore,
    pub strict_wer: ErrorTotal,
    /// WER, когда числа с обеих сторон приведены к цифрам.
    pub digits_wer: ErrorTotal,
    /// WER, когда слова через дефис склеены, а междометия не считаются.
    pub lenient_wer: ErrorTotal,
    pub processing_sec: f64,
    /// Время обработки клипов к их длительности на одно ядро движка.
    pub rtf: Option<f64>,
    pub failures: usize,
    /// Клипы, где конвейер нашёл больше одного спикера.
    pub multi_speaker: usize,
    /// Клипы, где конвейер не нашёл ни одного спикера: речи не найдено, текст пустой.
    pub no_speakers: usize,
    /// Клипы с известным числом голосов и те из них, где найдено ровно столько.
    pub speakers_known: usize,
    pub speakers_matched: usize,
    /// DER по клипам с разметкой спикеров; None, если таких клипов нет.
    pub der: Option<ClipDer>,
    pub by_duration: Vec<GroupScore>,
    pub with_digits: GroupScore,
    pub without_digits: GroupScore,
    /// Причины, по которым текст не получен, и сколько раз каждая встретилась.
    pub failure_reasons: BTreeMap<String, usize>,
    pub worst: Vec<ClipResult>,
    /// Самые частые замены слов.
    pub substitutions: Vec<SubstitutionCount>,
}

impl QualitySummary {
    pub fn of(results: &[ClipResult]) -> Self {
        let all: Vec<&ClipResult> = results.iter().collect();
        let processing_sec: f64 = results.iter().map(|c| c.processing_sec).sum();
        let audio_sec: f64 = results.iter().map(|c| c.duration_sec).sum();
        let (with, without): (Vec<&ClipResult>, Vec<&ClipResult>) =
            all.iter().partition(|c| c.has_digits());
        let mut failure_reasons = BTreeMap::new();
        for reason in results.iter().filter_map(|c| c.failure.as_ref()) {
            *failure_reasons.entry(reason.clone()).or_insert(0) += 1;
        }
        Self {
            overall: GroupScore::of("все клипы".into(), &all),
            strict_wer: ErrorTotal::of(
                results
                    .iter()
                    .map(|c| (c.strict_word_errors, c.strict_words)),
            ),
            digits_wer: ErrorTotal::of(
                results.iter().map(|c| (c.digit_word_errors, c.digit_words)),
            ),
            lenient_wer: ErrorTotal::of(
                results
                    .iter()
                    .map(|c| (c.lenient_word_errors, c.lenient_words)),
            ),
            processing_sec,
            rtf: (audio_sec > 0.0).then(|| processing_sec / audio_sec),
            failures: results.iter().filter(|c| c.failure.is_some()).count(),
            multi_speaker: results.iter().filter(|c| c.speakers > 1).count(),
            no_speakers: results
                .iter()
                .filter(|c| c.failure.is_none() && c.speakers == 0)
                .count(),
            speakers_known: results
                .iter()
                .filter(|c| c.speakers_match().is_some())
                .count(),
            speakers_matched: results
                .iter()
                .filter(|c| c.speakers_match() == Some(true))
                .count(),
            der: results.iter().filter_map(|c| c.der).reduce(|a, b| a + b),
            by_duration: by_duration(&all),
            with_digits: GroupScore::of("в ответе есть цифры".into(), &with),
            without_digits: GroupScore::of("в ответе нет цифр".into(), &without),
            failure_reasons,
            worst: worst(results),
            substitutions: top_substitutions(results, TOP_SUBSTITUTIONS),
        }
    }
}

fn by_duration(all: &[&ClipResult]) -> Vec<GroupScore> {
    let bucket = |d: f64| DURATION_EDGES.iter().filter(|&&edge| d >= edge).count();
    let mut groups: Vec<Vec<&ClipResult>> = vec![Vec::new(); DURATION_EDGES.len() + 1];
    for clip in all {
        groups[bucket(clip.duration_sec)].push(clip);
    }
    groups
        .iter()
        .enumerate()
        .map(|(i, clips)| GroupScore::of(duration_label(i), clips))
        .collect()
}

fn duration_label(bucket: usize) -> String {
    match (
        bucket.checked_sub(1).map(|i| DURATION_EDGES[i]),
        DURATION_EDGES.get(bucket),
    ) {
        (None, Some(hi)) => format!("до {hi} с"),
        (Some(lo), Some(hi)) => format!("от {lo} до {hi} с"),
        (Some(lo), None) => format!("от {lo} с"),
        (None, None) => String::new(),
    }
}

/// Порядок «хуже раньше»: больше ошибок по словам, при равенстве больше доля ошибок,
/// затем id.
pub fn worse_first(a: &ClipResult, b: &ClipResult) -> std::cmp::Ordering {
    // Ошибки без ошибок записи: клип, где «десять» распознано как «10», а «щас» как
    // «сейчас», не худший.
    b.relaxed_word_errors
        .cmp(&a.relaxed_word_errors)
        .then_with(|| {
            let rate = |c: &ClipResult| c.relaxed_wer().unwrap_or(0.0);
            rate(b).total_cmp(&rate(a))
        })
        .then_with(|| b.word_errors.cmp(&a.word_errors))
        .then_with(|| a.id.cmp(&b.id))
}

fn worst(results: &[ClipResult]) -> Vec<ClipResult> {
    let mut sorted: Vec<&ClipResult> = results.iter().filter(|c| c.has_real_errors()).collect();
    sorted.sort_by(|a, b| worse_first(a, b));
    sorted.into_iter().take(WORST_CLIPS).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClipHypothesis;

    fn clip(id: &str, reference: &str, heard: &str, duration: f64, speakers: u32) -> ClipResult {
        ClipResult::new(
            id,
            reference,
            duration,
            duration / 10.0,
            Ok(ClipHypothesis {
                text: heard.into(),
                speakers,
            }),
        )
    }

    #[test]
    fn errors_are_summed_over_clips_not_averaged() {
        let results = [
            clip("a", "раз два три четыре", "раз два три четыре", 2.0, 1),
            clip("b", "пять", "шесть", 4.0, 1),
        ];
        let s = QualitySummary::of(&results);
        assert_eq!(s.overall.wer.errors, 1);
        assert_eq!(s.overall.wer.reference, 5);
        assert_eq!(s.overall.wer.rate, Some(0.2));
        assert!((s.rtf.unwrap() - 0.1).abs() < 1e-12);
    }

    #[test]
    fn clips_fall_into_duration_groups_by_their_length() {
        let results = [
            clip("a", "раз", "раз", 2.9, 1),
            clip("b", "раз", "раз", 3.0, 1),
            clip("c", "раз", "раз", 7.9, 1),
            clip("d", "раз", "раз", 12.0, 1),
        ];
        let s = QualitySummary::of(&results);
        let counts: Vec<(String, usize)> = s
            .by_duration
            .iter()
            .map(|g| (g.label.clone(), g.clips))
            .collect();
        assert_eq!(
            counts,
            [
                ("до 3 с".to_string(), 1),
                ("от 3 до 5 с".to_string(), 1),
                ("от 5 до 8 с".to_string(), 1),
                ("от 8 с".to_string(), 1),
            ]
        );
    }

    #[test]
    fn clips_with_digits_are_scored_apart() {
        let results = [
            clip("a", "две тысячи", "2000", 2.0, 1),
            clip("b", "раз", "раз", 2.0, 1),
        ];
        let s = QualitySummary::of(&results);
        assert_eq!(s.with_digits.clips, 1);
        assert_eq!(s.digits_wer.errors, 0, "2000 и «две тысячи» совпадают");
        assert_eq!(s.without_digits.wer.rate, Some(0.0));
    }

    #[test]
    fn extra_speakers_failures_and_empty_text_are_counted() {
        let results = [
            clip("a", "раз", "раз", 2.0, 2),
            ClipResult::new("b", "раз", 2.0, 0.1, Err("нет речи".into())),
            ClipResult::new("c", "раз", 2.0, 0.1, Err("нет речи".into())),
        ];
        let s = QualitySummary::of(&results);
        assert_eq!(s.multi_speaker, 1);
        assert_eq!(s.no_speakers, 0, "ошибка не значит, что спикеров нет");
        assert_eq!(s.speakers_known, 0);
        assert_eq!(s.failures, 2);
        assert_eq!(s.overall.empty, 2);
        assert_eq!(s.failure_reasons.get("нет речи"), Some(&2));
    }

    #[test]
    fn clips_without_any_speaker_and_frequent_substitutions_are_counted() {
        let results = [
            clip("a", "раз", "", 2.0, 0),
            clip("b", "иду домой", "еду домой", 2.0, 1),
            clip("c", "иду", "еду", 2.0, 1),
        ];
        let s = QualitySummary::of(&results);
        assert_eq!(s.no_speakers, 1);
        assert_eq!(s.substitutions.len(), 1);
        assert_eq!(s.substitutions[0].count, 2);
    }

    #[test]
    fn der_is_summed_only_over_clips_with_speaker_truth() {
        let der = |missed: f64| {
            Some(ClipDer {
                reference_speech: 10.0,
                missed,
                false_alarm: 0.0,
                confusion: 0.0,
            })
        };
        let results = [
            clip("a", "раз", "раз", 2.0, 2).with_der(der(1.0)),
            clip("b", "раз", "раз", 2.0, 2).with_der(der(3.0)),
            clip("c", "раз", "раз", 2.0, 1),
        ];
        assert_eq!(QualitySummary::of(&results).der.unwrap().rate(), Some(0.2));
        assert_eq!(QualitySummary::of(&results[2..]).der, None);
    }

    #[test]
    fn lenient_wer_ignores_hyphens_and_fillers() {
        let results = [clip("a", "эээ какието вещи", "Какие-то вещи.", 2.0, 1)];
        let s = QualitySummary::of(&results);
        assert_eq!(s.overall.wer.errors, 2);
        assert_eq!((s.lenient_wer.errors, s.lenient_wer.reference), (0, 3));
    }

    #[test]
    fn found_voices_are_checked_only_where_the_true_number_is_known() {
        let results = [
            clip("a", "раз", "раз", 2.0, 2).from_dataset("calls", Some(2)),
            clip("b", "раз", "раз", 2.0, 3).from_dataset("calls", Some(2)),
            clip("c", "раз", "раз", 2.0, 1).from_dataset("talk", None),
            ClipResult::new("d", "раз", 2.0, 0.1, Err("нет".into())).from_dataset("calls", Some(2)),
        ];
        let s = QualitySummary::of(&results);
        assert_eq!((s.speakers_known, s.speakers_matched), (2, 1));
    }

    #[test]
    fn worst_clips_go_by_errors_then_rate_then_id_and_skip_perfect_ones() {
        let results = [
            clip("c", "раз два", "три четыре", 2.0, 1),
            clip("b", "раз два три четыре", "раз пять шесть четыре", 2.0, 1),
            clip("a", "раз", "раз", 2.0, 1),
            clip("d", "раз", "два", 2.0, 1),
        ];
        let ids: Vec<String> = QualitySummary::of(&results)
            .worst
            .iter()
            .map(|c| c.id.clone())
            .collect();
        assert_eq!(ids, ["c", "b", "d"]);
    }

    #[test]
    fn a_number_written_in_digits_is_not_a_worst_clip_but_a_real_error_is() {
        let results = [
            clip("cash", "сто тридцать пять рублей", "135 ₽", 2.0, 1),
            clip(
                "room",
                "в триста четырнадцатый кабинет",
                "в 314-й кабинет",
                2.0,
                1,
            ),
            clip("wrong", "десять ударов", "двадцать ударов", 2.0, 1),
        ];
        let summary = QualitySummary::of(&results);
        let ids: Vec<&str> = summary.worst.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["wrong"]);
        assert!(summary.substitutions.iter().all(|s| s.reference == "10"));
    }

    #[test]
    fn a_clip_without_errors_by_the_main_score_is_never_a_worst_clip() {
        let results = [
            clip("apart", "что то быстро", "Что-то быстро.", 2.0, 1),
            clip("joined", "чтото быстро", "что-то быстро", 2.0, 1),
            clip("wrong", "что то быстро", "что-то медленно", 2.0, 1),
        ];
        let summary = QualitySummary::of(&results);
        let ids: Vec<&str> = summary.worst.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["wrong"]);
    }
}
