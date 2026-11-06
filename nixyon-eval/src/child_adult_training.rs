use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use nixyon_ort::{ChildAdult, ChildAdultFeatures};

use std::collections::BTreeMap;

use crate::{
    FeatureClip, balanced_weights, choose_confidence, fit_logistic, speaker_centroid, speaker_folds,
};

/// Штрафы L2, из которых выбирается лучший по перекрёстной проверке.
pub const L2_GRID: [f64; 6] = [1e-4, 1e-3, 1e-2, 3e-2, 1e-1, 1.0];
pub const FOLDS: usize = 5;

/// Обученный классификатор и то, как он выбран: штраф с наименьшей потерей на клипах, не
/// видевших обучения (части по говорящим), и порог уверенности по ответам на говорящих из
/// отложенных частей: приложение подписывает спикера по сводным признакам его кусков.
#[derive(Debug, Clone)]
pub struct TrainedChildAdult {
    pub model: ChildAdult,
    pub l2: f64,
    /// Потеря на отложенных клипах для каждого штрафа из сетки, по порядку.
    pub l2_losses: Vec<f64>,
    /// Вероятность ребёнка у каждого обучающего клипа от модели, которая его не видела.
    pub held_out: Vec<f32>,
    /// Ребёнок ли и вероятность ребёнка у каждого обучающего говорящего по сводным признакам
    /// от модели, которая его не видела.
    pub held_out_speakers: Vec<(bool, f32)>,
    /// Найден ли порог, при котором взрослых названо детьми не больше допустимого.
    pub confidence_found: bool,
}

/// Обучает классификатор на `train` с перекрёстной проверкой по говорящим. Обучения идут
/// параллельно в `jobs` потоках. `max_adults_as_child`: допустимая доля взрослых, названных
/// детьми, для выбора порога.
pub fn train_child_adult(
    kind: ChildAdultFeatures,
    train: &[&FeatureClip],
    max_adults_as_child: f64,
    jobs: usize,
) -> Result<TrainedChildAdult, String> {
    let labels: Vec<(&str, bool)> = train
        .iter()
        .map(|c| (c.speaker.as_str(), c.child))
        .collect();
    let folds = speaker_folds(&labels, FOLDS);
    let weights = balanced_weights(&labels);
    let tasks: Vec<(usize, usize)> = (0..L2_GRID.len())
        .flat_map(|l| (0..FOLDS).map(move |f| (l, f)))
        .collect();
    let held_out = Mutex::new(vec![vec![0.0f32; train.len()]; L2_GRID.len()]);
    let held_out_speakers = Mutex::new(vec![Vec::new(); L2_GRID.len()]);
    let failure = Mutex::new(None);
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| {
                while let Some(&(l, f)) = tasks.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let fit_on: Vec<usize> = (0..train.len()).filter(|i| folds[*i] != f).collect();
                    let result = fit_subset(kind, train, &fit_on, L2_GRID[l]).and_then(|m| {
                        let held: Vec<usize> =
                            (0..train.len()).filter(|i| folds[*i] == f).collect();
                        let clips = held
                            .iter()
                            .map(|&i| Ok((i, child_p(&m, train[i])?)))
                            .collect::<Result<Vec<_>, String>>()?;
                        Ok((clips, speaker_ps(&m, train, &held)?))
                    });
                    match result {
                        Ok((clips, speakers)) => {
                            if let Ok(mut h) = held_out.lock() {
                                for (i, p) in clips {
                                    h[l][i] = p;
                                }
                            }
                            if let Ok(mut h) = held_out_speakers.lock() {
                                h[l].extend(speakers);
                            }
                        }
                        Err(e) => {
                            if let Ok(mut slot) = failure.lock() {
                                slot.get_or_insert(e);
                            }
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = failure.into_inner().map_err(|_| "сбой потока обучения")? {
        return Err(e);
    }
    let held_out = held_out.into_inner().map_err(|_| "сбой потока обучения")?;
    let l2_losses: Vec<f64> = held_out
        .iter()
        .map(|ps| log_loss(ps, train, &weights))
        .collect();
    let best = l2_losses
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i)
        .ok_or("пустая сетка штрафов")?;
    let held_out = held_out.into_iter().nth(best).unwrap_or_default();
    let mut held_out_speakers = held_out_speakers
        .into_inner()
        .map_err(|_| "сбой потока обучения")?
        .into_iter()
        .nth(best)
        .unwrap_or_default();
    // Порядок говорящих зависит от того, какой поток закончил раньше.
    held_out_speakers.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let confidence = choose_confidence(&held_out_speakers, max_adults_as_child);
    let all: Vec<usize> = (0..train.len()).collect();
    let mut model = fit_subset(kind, train, &all, L2_GRID[best])?;
    // Без подходящего порога классификатор всегда отвечает «не определено».
    model.confidence = confidence.unwrap_or(1.0);
    Ok(TrainedChildAdult {
        model,
        l2: L2_GRID[best],
        l2_losses,
        held_out,
        held_out_speakers,
        confidence_found: confidence.is_some(),
    })
}

fn fit_subset(
    kind: ChildAdultFeatures,
    train: &[&FeatureClip],
    subset: &[usize],
    l2: f64,
) -> Result<ChildAdult, String> {
    let labels: Vec<(&str, bool)> = subset
        .iter()
        .map(|i| (train[*i].speaker.as_str(), train[*i].child))
        .collect();
    let weights = balanced_weights(&labels);
    let rows: Vec<&[f32]> = subset.iter().map(|i| train[*i].features(kind)).collect();
    let child: Vec<bool> = subset.iter().map(|i| train[*i].child).collect();
    fit_logistic(kind, &rows, &child, &weights, l2)
}

/// Вероятность ребёнка у каждого говорящего из `subset` по сводным признакам его клипов.
fn speaker_ps(
    model: &ChildAdult,
    train: &[&FeatureClip],
    subset: &[usize],
) -> Result<Vec<(bool, f32)>, String> {
    let mut groups: BTreeMap<&str, Vec<&FeatureClip>> = BTreeMap::new();
    for &i in subset {
        groups
            .entry(train[i].speaker.as_str())
            .or_default()
            .push(train[i]);
    }
    let mut out = Vec::with_capacity(groups.len());
    for clips in groups.values() {
        let parts: Vec<(&[f32], f64)> = clips
            .iter()
            .map(|c| (c.features(model.features), c.duration_sec))
            .collect();
        if let Some(centroid) = speaker_centroid(model.features, &parts) {
            let p = model
                .child_probability(&centroid)
                .map_err(|e| e.to_string())?;
            out.push((clips[0].child, p));
        }
    }
    Ok(out)
}

fn child_p(model: &ChildAdult, clip: &FeatureClip) -> Result<f32, String> {
    model
        .child_probability(clip.features(model.features))
        .map_err(|e| e.to_string())
}

/// Взвешенная логистическая потеря; вероятность прижата к (1e-7, 1 - 1e-7), чтобы уверенная
/// ошибка не давала бесконечность.
fn log_loss(ps: &[f32], clips: &[&FeatureClip], weights: &[f64]) -> f64 {
    ps.iter()
        .zip(clips)
        .zip(weights)
        .map(|((p, c), w)| {
            let p = f64::from(*p).clamp(1e-7, 1.0 - 1e-7);
            -w * if c.child { p.ln() } else { (1.0 - p).ln() }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FeatureRole;

    fn clip(speaker: &str, child: bool, x: f32) -> FeatureClip {
        FeatureClip {
            role: FeatureRole::Train,
            set: "t".into(),
            speaker: speaker.into(),
            child,
            duration_sec: 1.0,
            redimnet2: vec![x, 0.5],
            age_gender_hidden: vec![x],
        }
    }

    #[test]
    fn a_separable_set_trains_a_classifier_with_a_found_confidence() {
        let mut clips = Vec::new();
        for s in 0..10 {
            for k in 0..3 {
                let shift = k as f32 * 0.1;
                clips.push(clip(&format!("c{s}"), true, 2.0 + shift));
                clips.push(clip(&format!("a{s}"), false, -2.0 - shift));
            }
        }
        let refs: Vec<&FeatureClip> = clips.iter().collect();
        let t = train_child_adult(ChildAdultFeatures::Redimnet2, &refs, 0.02, 4).unwrap();
        assert!(t.confidence_found);
        assert_eq!(t.l2_losses.len(), L2_GRID.len());
        assert_eq!(t.held_out.len(), clips.len());
        assert_eq!(t.held_out_speakers.len(), 20);
        for (c, p) in clips.iter().zip(&t.held_out) {
            assert_eq!(*p >= 0.5, c.child);
        }
        assert_eq!(t.model.verdict(&[2.5, 0.5]).unwrap(), Some(true));
    }
}
