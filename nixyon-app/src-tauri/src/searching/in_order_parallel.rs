//! Обработка списка на всех ядрах с сохранением порядка: поиск по архиву читает и
//! просматривает встречи независимо друг от друга.

use std::num::NonZeroUsize;

/// Результаты в том же порядке, что `items`. Паника в одном потоке поднимается дальше.
pub fn map_in_order<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    if items.is_empty() {
        return Vec::new();
    }
    let threads = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let chunk = items.len().div_ceil(threads.min(items.len()));
    let f = &f;
    std::thread::scope(|s| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(f).collect::<Vec<R>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_keep_the_order_of_the_items() {
        let items: Vec<u32> = (0..1000).collect();
        assert_eq!(
            map_in_order(&items, |x| x * 2),
            items.iter().map(|x| x * 2).collect::<Vec<_>>()
        );
        assert_eq!(map_in_order(&[] as &[u32], |x| *x), Vec::<u32>::new());
        assert_eq!(map_in_order(&[7u32], |x| *x), [7]);
    }
}
