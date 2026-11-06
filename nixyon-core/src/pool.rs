use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, TryLockError};

/// Несколько одинаковых ресурсов (сессий модели), каждый под своим замком: вызовы с разных
/// потоков идут параллельно, пока свободен хоть один. Ресурс берётся по кругу; если все
/// заняты, вызов ждёт того, на кого выпала очередь.
pub struct Pool<T> {
    items: Vec<Mutex<T>>,
    next: AtomicUsize,
}

/// Ресурс пула испорчен: поток упал, пока держал его.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("ресурс пула испорчен упавшим потоком")]
pub struct PoolPoisoned;

impl<T> Pool<T> {
    /// Пул из одного ресурса: вызовы идут по одному, как с простым замком.
    pub fn single(item: T) -> Self {
        Self::with_first(item, Vec::new())
    }

    /// Пул из первого ресурса и остальных.
    pub fn with_first(first: T, rest: Vec<T>) -> Self {
        Self {
            items: std::iter::once(first).chain(rest).map(Mutex::new).collect(),
            next: AtomicUsize::new(0),
        }
    }

    /// None: пул без ресурсов.
    pub fn new(items: Vec<T>) -> Option<Self> {
        if items.is_empty() {
            return None;
        }
        Some(Self {
            items: items.into_iter().map(Mutex::new).collect(),
            next: AtomicUsize::new(0),
        })
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Выполняет `f` со свободным ресурсом пула.
    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R, PoolPoisoned> {
        let mut guard = self.take()?;
        Ok(f(&mut guard))
    }

    fn take(&self) -> Result<MutexGuard<'_, T>, PoolPoisoned> {
        let n = self.items.len();
        let start = self.next.fetch_add(1, Ordering::Relaxed) % n;
        for i in 0..n {
            match self.items[(start + i) % n].try_lock() {
                Ok(guard) => return Ok(guard),
                Err(TryLockError::WouldBlock) => continue,
                Err(TryLockError::Poisoned(_)) => return Err(PoolPoisoned),
            }
        }
        self.items[start].lock().map_err(|_| PoolPoisoned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    #[test]
    fn empty_pool_is_refused() {
        assert!(Pool::<u8>::new(Vec::new()).is_none());
    }

    #[test]
    fn calls_from_many_threads_use_different_resources_at_once() {
        let pool = Pool::new(vec![0usize; 3]).unwrap();
        let barrier = Barrier::new(3);
        std::thread::scope(|s| {
            for _ in 0..3 {
                s.spawn(|| {
                    pool.with(|count| {
                        *count += 1;
                        // Все три потока держат ресурсы одновременно: иначе барьер не пройти.
                        barrier.wait();
                    })
                    .unwrap();
                });
            }
        });
        let counts: Vec<usize> = (0..3).map(|_| pool.with(|c| *c).unwrap()).collect();
        assert_eq!(counts.iter().sum::<usize>(), 3);
        assert!(counts.iter().all(|&c| c == 1), "{counts:?}");
    }

    #[test]
    fn busy_pool_waits_instead_of_failing() {
        let pool = Pool::new(vec![0u32]).unwrap();
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| pool.with(|n| *n += 1).unwrap());
            }
        });
        assert_eq!(pool.with(|n| *n).unwrap(), 8);
    }
}
