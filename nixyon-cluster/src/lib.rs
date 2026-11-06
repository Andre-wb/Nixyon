//! Кластеризация эмбеддингов голоса: AHC, оценка числа спикеров по eigengap, центроиды,
//! пересегментация Витерби по окнам, сопоставление один к одному (венгерский алгоритм).
//! Работает с единичными векторами: похожесть голосов меряется косинусом.

mod ahc;
mod assignment;
mod centroids;
mod sized_clusters;
mod speaker_count;
mod unit_vector;
mod viterbi;

#[cfg(test)]
mod test_support;

pub use ahc::ahc;
pub use assignment::min_cost_assignment;
pub use centroids::{centroids, nearest};
pub use sized_clusters::sized_ahc;
pub use speaker_count::{SAME_SPEAKER_CENTROID_COSINE, estimate_speaker_count};
pub use unit_vector::{cosine, unit};
pub use viterbi::viterbi;
