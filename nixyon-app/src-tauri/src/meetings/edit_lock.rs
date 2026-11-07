//! Правки файлов встречи (расшифровки и центров голосов) идут по одной: каждая читает
//! файл, меняет и записывает, и без блокировки одна из двух одновременных правок
//! потерялась бы. Под этой блокировкой можно брать блокировку индекса, но не наоборот,
//! иначе две команды могут ждать друг друга вечно.

use std::sync::{Mutex, PoisonError};

static EDIT_LOCK: Mutex<()> = Mutex::new(());

pub fn locked<T>(work: impl FnOnce() -> T) -> T {
    let _guard = EDIT_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    work()
}
