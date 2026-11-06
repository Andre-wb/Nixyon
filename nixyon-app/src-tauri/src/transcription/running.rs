use std::sync::Mutex;

use nixyon_core::CancelFlag;

/// Идущая обработка: одна за раз, её флаг отмены доступен команде отмены.
#[derive(Default)]
pub struct Running {
    current: Mutex<Option<CancelFlag>>,
}

/// Пока жив, обработка считается идущей; при выходе из команды место освобождается
/// при любом исходе.
pub struct Slot<'a> {
    owner: &'a Running,
    pub cancel: CancelFlag,
}

impl Running {
    pub fn begin(&self) -> Option<Slot<'_>> {
        let mut current = self.current.lock().ok()?;
        if current.is_some() {
            return None;
        }
        let cancel = CancelFlag::new();
        *current = Some(cancel.clone());
        Some(Slot {
            owner: self,
            cancel,
        })
    }

    /// true, если было что отменять.
    pub fn cancel(&self) -> bool {
        match self.current.lock().ok().and_then(|c| c.clone()) {
            Some(flag) => {
                flag.cancel();
                true
            }
            None => false,
        }
    }
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        if let Ok(mut current) = self.owner.current.lock() {
            *current = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_run_at_a_time_and_cancel_reaches_it() {
        let running = Running::default();
        assert!(!running.cancel());
        let slot = running.begin().unwrap();
        assert!(running.begin().is_none());
        assert!(running.cancel());
        assert!(slot.cancel.is_cancelled());
        drop(slot);
        let next = running.begin().unwrap();
        assert!(!next.cancel.is_cancelled());
    }
}
