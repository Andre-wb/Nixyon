//! Модели, которые sherpa-onnx не загружает, на крейте ort: модель голоса ReDimNet2,
//! нейросегментация Sortformer, пол с возрастом по голосу (audEERING), классификатор
//! «ребёнок или взрослый» поверх признаков голоса и эмоция реплики (GigaAM-Emo). ONNX
//! Runtime общий с sherpa-onnx, второй копии нет.

mod age_gender;
mod binarize;
mod bundled_child_adult;
mod child_adult;
mod child_adult_features;
mod emotion_model;
mod emotion_probs;
mod error;
mod log_mel;
mod manifest;
mod mean_unit;
mod median;
mod mel_filterbank;
mod redimnet;
mod redimnet_input;
mod repeat_pad;
mod runtime;
mod softmax;
mod sortformer;
mod speaker_child_verdict;
mod speaker_voice;
mod voice_analysis;
mod voice_traits;
mod weighted_vector_mean;

pub use age_gender::AgeGender;
pub use binarize::Binarize;
pub use bundled_child_adult::bundled_child_adult;
pub use child_adult::ChildAdult;
pub use child_adult_features::ChildAdultFeatures;
pub use emotion_model::EmotionModel;
pub use emotion_probs::EmotionProbs;
pub use error::OrtError;
pub use manifest::{OrtManifest, OrtRole};
pub use redimnet::Redimnet2;
pub use sortformer::Sortformer;
pub use speaker_child_verdict::speaker_child_verdict;
pub use speaker_voice::{SpeakerVoice, speaker_voice};
pub use voice_analysis::VoiceAnalysis;
pub use voice_traits::VoiceTraits;
