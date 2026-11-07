use serde::{Deserialize, Serialize};

/// Метка из общего списка индекса. Встречи ссылаются на неё по `id`, поэтому
/// переименование меняет одно место. Название вводит человек, в лог оно не пишется.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub id: u32,
    pub name: String,
}
