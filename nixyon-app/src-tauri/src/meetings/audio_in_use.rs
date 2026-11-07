//! Встреча, чей звук сейчас слушается заново для пересчёта реплики. Очистка звука её
//! пропускает: иначе запись пропала бы посреди работы. Пересчёт идёт один за раз, поэтому
//! хватает одного места.

use std::sync::{Mutex, PoisonError};

use super::meeting_id::MeetingId;

static IN_USE: Mutex<Option<MeetingId>> = Mutex::new(None);

/// Пока жива, звук встречи считается занятым; при выходе из команды освобождается при
/// любом исходе.
pub struct AudioInUse(());

impl AudioInUse {
    pub fn hold(id: MeetingId) -> Self {
        *IN_USE.lock().unwrap_or_else(PoisonError::into_inner) = Some(id);
        Self(())
    }
}

impl Drop for AudioInUse {
    fn drop(&mut self) {
        *IN_USE.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

pub fn is_in_use(id: &MeetingId) -> bool {
    IN_USE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        == Some(id)
}
