use std::sync::atomic::{AtomicUsize, Ordering};

/// `f` от каждого элемента в `jobs` потоков; результаты в порядке элементов. При одном
/// потоке или одном элементе всё считается в вызывающем потоке.
pub fn par_map<T, R, F>(items: &[T], jobs: usize, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let workers = jobs.min(items.len());
    if workers <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else {
                            break;
                        };
                        mine.push((i, f(item)));
                    }
                    mine
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| match h.join() {
                Ok(mine) => mine,
                Err(panic) => std::panic::resume_unwind(panic),
            })
            .collect()
    });
    done.sort_unstable_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_keep_the_order_of_items_for_any_number_of_jobs() {
        let items: Vec<u64> = (0..1000).collect();
        let expected: Vec<u64> = items.iter().map(|x| x * x).collect();
        for jobs in [0, 1, 2, 7, 64, 5000] {
            assert_eq!(par_map(&items, jobs, |x| x * x), expected, "{jobs}");
        }
        assert!(par_map(&[] as &[u8], 4, |x| *x).is_empty());
    }

    #[test]
    fn work_is_spread_over_several_threads() {
        let items = vec![(); 64];
        let ids = par_map(&items, 4, |_| {
            std::thread::sleep(std::time::Duration::from_millis(2));
            std::thread::current().id()
        });
        let mut distinct = ids.clone();
        distinct.sort_by_key(|id| format!("{id:?}"));
        distinct.dedup();
        assert!(distinct.len() > 1, "{distinct:?}");
    }
}
