use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Флаг отмены долгой обработки. Клоны смотрят на один и тот же флаг.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    // Relaxed достаточно: флаг ничего не публикует, кроме самого себя, и
    // обработка проверяет его регулярно, так что отмена увидится на ближайшей проверке.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_seen_through_every_clone() {
        let flag = CancelFlag::new();
        let other = flag.clone();
        assert!(!other.is_cancelled());
        flag.cancel();
        assert!(other.is_cancelled());
    }

    #[test]
    fn cancel_from_another_thread_is_seen() {
        let flag = CancelFlag::new();
        let remote = flag.clone();
        std::thread::spawn(move || remote.cancel()).join().unwrap();
        assert!(flag.is_cancelled());
    }
}
