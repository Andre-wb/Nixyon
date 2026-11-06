use serde::{Deserialize, Serialize};

use crate::{SpeakerAge, SpeakerGender};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Speaker {
    /// Номер с 1. После слияния спикеров номера не перенумеровываются.
    pub id: u32,
    /// None: при выводе подставляется «Спикер N» на языке интерфейса.
    pub name: Option<String>,
    /// Статистика пересчитывается после каждой правки.
    pub talk_time_sec: f64,
    pub turns: u32,
    pub words: u32,
    /// Ребёнок или взрослый по голосу; None, если модели нет или она не уверена.
    /// default: файлы без поля тоже читаются.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age: Option<SpeakerAge>,
    /// Пол по голосу; None, если модели нет или ответа не было. default: файлы без поля
    /// тоже читаются.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<SpeakerGender>,
}
