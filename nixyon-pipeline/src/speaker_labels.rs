use nixyon_cluster::{ahc, centroids, estimate_speaker_count, nearest, sized_ahc, unit};
use nixyon_core::SegmentResult;

use crate::resegment::resegment;
use crate::sample_names::SpeakerCenter;
use crate::{PipelineOptions, SpeakerCount, WindowLabel};

/// Значение по умолчанию для `PipelineOptions::long_segment_sec`: куски от этой
/// длины надёжно описывают голос.
pub const LONG_SEGMENT_SEC: f64 = 1.5;
/// Значение по умолчанию для `PipelineOptions::max_auto_speakers`.
pub const MAX_AUTO_SPEAKERS: usize = 8;
/// Значение по умолчанию для `PipelineOptions::min_speaker_sec`. По замеру на 8 диалогах:
/// 3 с не убирают шумовые кластеры, 5 с отнимают голос, который почти весь звучит
/// поверх чужого.
pub const MIN_SPEAKER_SEC: f64 = 4.0;

/// Запись короткая, если в кусках от `long_segment_sec` меньше этого времени речи. Тогда
/// голос того, кто отвечает коротко, почти весь в коротких кусках: без них его нет в
/// кластеризации, и все его реплики достаются собеседнику.
const SHORT_RECORDING_SPEECH_SEC: f64 = 20.0;
/// В короткой записи в кластеризацию идут куски от этой длины. По замеру на 5 коротких
/// синтезированных диалогах и 2 монологах 1 с находит второго спикера там же, где 0,8 с,
/// но в диалоге трёх человек на 15 с ошибается чаще (DER 62 % против 36 %); монологи
/// остаются одним спикером, на 30-секундных отрывках AMI оба порога ничего не меняют.
const SHORT_RECORDING_SEGMENT_SEC: f64 = 0.8;
/// Спикеру хватает этой доли речи кластеризуемых кусков, если она меньше
/// `min_speaker_sec`: в записи на 20 с у собеседника может не набраться 4 с речи.
const MIN_SPEAKER_SHARE: f64 = 0.25;

/// Номера спикеров с 1 в порядке первого появления: у каждого куска и, если работала
/// пересегментация, у окон внутри куска.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerLabels {
    pub segments: Vec<u32>,
    pub windows: Vec<Vec<WindowLabel>>,
    /// Центр голоса каждого спикера, у которого он есть.
    pub(crate) centers: Vec<SpeakerCenter>,
}

/// Разметка спикеров. Куски идут по времени. Длинные куски кластеризуются, короткие
/// уходят к ближайшему центру; при включённом Витерби или разрезе по словам метки
/// уточняются по окнам.
pub fn label_segments(
    segments: &[SegmentResult],
    count: SpeakerCount,
    options: &PipelineOptions,
) -> SpeakerLabels {
    let (mut labels, centers) = cluster(segments, count, options);
    let mut window_labels: Vec<Vec<(f64, f64, usize)>> = vec![Vec::new(); segments.len()];
    if let Some(penalty) = options.window_penalty()
        && !centers.is_empty()
    {
        let r = resegment(segments, &labels, &centers, penalty);
        labels = r.segment_labels;
        window_labels = r.window_labels;
    }
    fill_from_time_neighbours(&mut labels);
    numbered(&labels, &window_labels, &centers)
}

/// Метка кластера у каждого куска (None у куска без голоса) и центры кластеров.
fn cluster(
    segments: &[SegmentResult],
    count: SpeakerCount,
    options: &PipelineOptions,
) -> (Vec<Option<usize>>, Vec<Option<Vec<f32>>>) {
    let durations: Vec<f64> = segments.iter().map(|s| s.end - s.start).collect();
    let units: Vec<Option<Vec<f32>>> = segments.iter().map(|s| unit(&s.embedding)).collect();
    cluster_units(&durations, &units, count, options)
}

/// Кластеризация отрезков голоса по длительности и единичному эмбеддингу (None: голоса
/// нет). Длинные отрезки кластеризуются, короткие уходят к ближайшему центру. Итог:
/// метка каждого отрезка и центры кластеров.
pub(crate) fn cluster_units(
    durations: &[f64],
    units: &[Option<Vec<f32>>],
    count: SpeakerCount,
    options: &PipelineOptions,
) -> (Vec<Option<usize>>, Vec<Option<Vec<f32>>>) {
    let with_voice: Vec<usize> = (0..units.len()).filter(|&i| units[i].is_some()).collect();
    let long_sec = clustered_from(durations, &with_voice, options.long_segment_sec);
    let long: Vec<usize> = with_voice
        .iter()
        .copied()
        .filter(|&i| durations[i] >= long_sec)
        .collect();
    let base = if long.is_empty() { with_voice } else { long };
    if base.is_empty() {
        return (vec![None; units.len()], Vec::new());
    }

    let base_units: Vec<Vec<f32>> = base.iter().filter_map(|&i| units[i].clone()).collect();
    let k = match count {
        SpeakerCount::Known(k) => usize::try_from(k)
            .unwrap_or(usize::MAX)
            .clamp(1, base_units.len()),
        SpeakerCount::Auto => estimate_speaker_count(
            &base_units,
            options.max_auto_speakers,
            options.same_speaker_cosine,
        ),
    };
    let base_labels = match count {
        SpeakerCount::Known(_) => ahc(&base_units, k),
        SpeakerCount::Auto => {
            let base_durations: Vec<f64> = base.iter().map(|&i| durations[i]).collect();
            let min_sec = options
                .min_speaker_sec
                .min(MIN_SPEAKER_SHARE * base_durations.iter().sum::<f64>());
            sized_ahc(
                &base_units,
                &base_durations,
                k,
                min_sec,
                options.same_speaker_cosine,
            )
        }
    };
    let k = base_labels.iter().max().map_or(0, |&m| m + 1);
    let centers = centroids(&base_units, &base_labels, k);

    let mut labels: Vec<Option<usize>> = vec![None; units.len()];
    for (&i, &label) in base.iter().zip(&base_labels) {
        labels[i] = Some(label);
    }
    for (i, u) in units.iter().enumerate() {
        if labels[i].is_none()
            && let Some(u) = u
        {
            labels[i] = nearest(&centers, u);
        }
    }
    (labels, centers)
}

// Длина, с которой кусок с голосом идёт в кластеризацию: `long_segment_sec`, а в
// короткой записи не больше SHORT_RECORDING_SEGMENT_SEC.
fn clustered_from(durations: &[f64], with_voice: &[usize], long_segment_sec: f64) -> f64 {
    let long_speech: f64 = with_voice
        .iter()
        .map(|&i| durations[i])
        .filter(|&d| d >= long_segment_sec)
        .sum();
    if long_speech < SHORT_RECORDING_SPEECH_SEC {
        long_segment_sec.min(SHORT_RECORDING_SEGMENT_SEC)
    } else {
        long_segment_sec
    }
}

// Кусок без голоса (слишком короткий для модели) берёт спикера соседа:
// предыдущего, а в начале записи следующего.
pub(crate) fn fill_from_time_neighbours(labels: &mut [Option<usize>]) {
    let mut previous = None;
    for label in labels.iter_mut() {
        match label {
            Some(l) => previous = Some(*l),
            None => *label = previous,
        }
    }
    let mut next = None;
    for label in labels.iter_mut().rev() {
        match label {
            Some(l) => next = Some(*l),
            None => *label = next,
        }
    }
}

// Номера по первому появлению во времени: внутри куска с окнами по окнам, иначе по куску.
fn numbered(
    labels: &[Option<usize>],
    windows: &[Vec<(f64, f64, usize)>],
    centers: &[Option<Vec<f32>>],
) -> SpeakerLabels {
    let mut order: Vec<usize> = Vec::new();
    let mut id = |l: usize| {
        let index = order.iter().position(|&o| o == l).unwrap_or_else(|| {
            order.push(l);
            order.len() - 1
        });
        u32::try_from(index + 1).unwrap_or(u32::MAX)
    };
    let mut segment_ids = Vec::with_capacity(labels.len());
    let mut window_ids = Vec::with_capacity(labels.len());
    for (label, w) in labels.iter().zip(windows) {
        let w: Vec<WindowLabel> = w
            .iter()
            .map(|&(start, end, l)| WindowLabel {
                start,
                end,
                speaker: id(l),
            })
            .collect();
        segment_ids.push(id(label.unwrap_or(0)));
        window_ids.push(w);
    }
    SpeakerLabels {
        segments: segment_ids,
        windows: window_ids,
        centers: centers_by_speaker(&order, centers),
    }
}

/// Центры по номерам спикеров: `order[i]` это кластер спикера `i + 1`.
pub(crate) fn centers_by_speaker(
    order: &[usize],
    centers: &[Option<Vec<f32>>],
) -> Vec<SpeakerCenter> {
    order
        .iter()
        .zip(1u32..)
        .filter_map(|(&label, id)| centers.get(label)?.clone().map(|c| (id, c)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{segment, voice};

    // Два похожих, но разных голоса по три куска в 2 с и одна шумовая точка вдали от обоих.
    fn two_voices_and_noise() -> (Vec<f64>, Vec<Option<Vec<f32>>>) {
        let v = |x: f32, y: f32, z: f32| unit(&[x, y, z, 0.0]);
        let units = vec![
            v(1.0, 0.0, 0.01),
            v(1.0, 0.01, 0.0),
            v(1.0, 0.0, -0.01),
            v(0.6, 0.8, 0.01),
            v(0.6, 0.8, 0.0),
            v(0.61, 0.8, 0.0),
            unit(&[0.0, 0.0, 0.0, 1.0]),
        ];
        (vec![2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0], units)
    }

    #[test]
    fn with_unknown_count_noise_does_not_take_the_place_of_a_voice() {
        let (durations, units) = two_voices_and_noise();
        let options = PipelineOptions::default();
        let k = |count| {
            let (labels, _) = cluster_units(&durations, &units, count, &options);
            labels.into_iter().flatten().collect::<Vec<_>>()
        };
        // Шум далёк от обоих голосов и занял бы место спикера; правило отдаёт его голосу.
        let auto = k(SpeakerCount::Auto);
        assert_ne!(auto[0], auto[3], "{auto:?}");
        assert_eq!(auto.iter().max(), Some(&1), "{auto:?}");
        // Заданное пользователем число не меняется: три группы, как просили.
        let known = k(SpeakerCount::Known(3));
        assert_eq!(known.iter().max(), Some(&2), "{known:?}");
        let off = PipelineOptions {
            min_speaker_sec: 0.0,
            ..PipelineOptions::default()
        };
        let (plain, _) = cluster_units(&durations, &units, SpeakerCount::Auto, &off);
        assert_eq!(
            plain[0], plain[3],
            "без правила шум занимает место, голоса сливаются"
        );
    }

    fn ids(segs: &[SegmentResult], count: SpeakerCount, options: &PipelineOptions) -> Vec<u32> {
        label_segments(segs, count, options).segments
    }

    #[test]
    fn two_voices_alternating_get_ids_in_order_of_first_appearance() {
        let segs = vec![
            segment(0.0, 2.0, "а", voice(1, 0.0)),
            segment(2.0, 4.0, "б", voice(2, 0.0)),
            segment(4.0, 6.0, "в", voice(1, 0.1)),
            segment(6.0, 8.0, "г", voice(2, 0.1)),
        ];
        assert_eq!(
            ids(&segs, SpeakerCount::Auto, &PipelineOptions::default()),
            [1, 2, 1, 2]
        );
        assert_eq!(
            ids(&segs, SpeakerCount::Known(2), &PipelineOptions::default()),
            [1, 2, 1, 2]
        );
    }

    #[test]
    fn max_auto_speakers_caps_the_estimate() {
        let segs = vec![
            segment(0.0, 2.0, "а", voice(1, 0.0)),
            segment(2.0, 4.0, "б", voice(2, 0.0)),
            segment(4.0, 6.0, "в", voice(1, 0.1)),
            segment(6.0, 8.0, "г", voice(2, 0.1)),
        ];
        let one = PipelineOptions {
            max_auto_speakers: 1,
            ..PipelineOptions::default()
        };
        assert_eq!(ids(&segs, SpeakerCount::Auto, &one), [1, 1, 1, 1]);
    }

    #[test]
    fn long_segment_threshold_decides_who_enters_clustering() {
        // Кусок третьего голоса короче порога даже короткой записи: по умолчанию он не
        // участвует в кластеризации и уходит к ближайшему из двух центров; с низким порогом
        // получает свою группу.
        let segs = vec![
            segment(0.0, 2.0, "а", voice(1, 0.0)),
            segment(2.0, 4.0, "б", voice(2, 0.0)),
            segment(4.0, 4.6, "в", voice(3, 0.0)),
            segment(5.0, 7.0, "г", voice(1, 0.1)),
            segment(7.0, 9.0, "д", voice(2, 0.1)),
        ];
        let by_default = ids(&segs, SpeakerCount::Known(3), &PipelineOptions::default());
        assert_ne!(by_default, [1, 2, 3, 1, 2]);
        let low = PipelineOptions {
            long_segment_sec: 0.5,
            ..PipelineOptions::default()
        };
        assert_eq!(ids(&segs, SpeakerCount::Known(3), &low), [1, 2, 3, 1, 2]);
    }

    #[test]
    fn in_a_short_recording_a_voice_needs_only_a_quarter_of_the_speech() {
        // Как в коротком диалоге: у первого голоса 3,8 с речи в длинных кусках, меньше
        // общих 4 с, но больше четверти речи записи.
        let segs = vec![
            segment(0.0, 1.52, "а", voice(1, 0.0)),
            segment(2.0, 6.58, "б", voice(2, 0.0)),
            segment(7.0, 9.26, "в", voice(1, 0.1)),
        ];
        let options = PipelineOptions::default();
        assert_eq!(ids(&segs, SpeakerCount::Auto, &options), [1, 2, 1]);
    }

    // Первый голос говорит кусками по 2 с, второй отвечает только кусками по 1 с.
    fn short_replies(long_pieces: usize) -> Vec<SegmentResult> {
        let mut segs = Vec::new();
        let mut t = 0.0;
        for i in 0..long_pieces {
            segs.push(segment(t, t + 2.0, "а", voice(1, 0.01 * i as f32)));
            t += 2.5;
            if i < 4 {
                segs.push(segment(t, t + 1.0, "б", voice(2, 0.01 * i as f32)));
                t += 1.5;
            }
        }
        segs
    }

    #[test]
    fn short_replies_get_their_own_speaker_in_a_short_recording() {
        let options = PipelineOptions::default();
        let short = ids(&short_replies(4), SpeakerCount::Auto, &options);
        assert_eq!(short, [1, 2, 1, 2, 1, 2, 1, 2]);
        // В длинной записи короткие куски по-прежнему уходят к ближайшему центру.
        let long = ids(&short_replies(11), SpeakerCount::Auto, &options);
        assert!(long.iter().all(|&id| id == 1), "{long:?}");
    }

    #[test]
    fn short_segment_joins_nearest_voice() {
        let segs = vec![
            segment(0.0, 2.0, "а", voice(1, 0.0)),
            segment(2.0, 4.0, "б", voice(2, 0.0)),
            segment(4.0, 4.5, "в", voice(1, 0.2)),
        ];
        assert_eq!(
            ids(&segs, SpeakerCount::Known(2), &PipelineOptions::default()),
            [1, 2, 1]
        );
    }

    #[test]
    fn segment_without_embedding_takes_previous_or_next_speaker() {
        let segs = vec![
            segment(0.0, 0.3, "э", vec![]),
            segment(0.5, 2.5, "а", voice(1, 0.0)),
            segment(3.0, 5.0, "б", voice(2, 0.0)),
            segment(5.0, 5.2, "м", vec![]),
        ];
        assert_eq!(
            ids(&segs, SpeakerCount::Known(2), &PipelineOptions::default()),
            [1, 1, 2, 2]
        );
    }

    #[test]
    fn known_count_larger_than_segments_is_clamped() {
        let segs = vec![segment(0.0, 2.0, "а", voice(1, 0.0))];
        assert_eq!(
            ids(&segs, SpeakerCount::Known(5), &PipelineOptions::default()),
            [1]
        );
        assert_eq!(
            ids(&segs, SpeakerCount::Known(0), &PipelineOptions::default()),
            [1]
        );
    }

    #[test]
    fn no_embeddings_at_all_gives_single_speaker() {
        let segs = vec![
            segment(0.0, 2.0, "а", vec![]),
            segment(2.0, 4.0, "б", vec![0.0; 4]),
        ];
        assert_eq!(
            ids(&segs, SpeakerCount::Auto, &PipelineOptions::default()),
            [1, 1]
        );
    }
}
