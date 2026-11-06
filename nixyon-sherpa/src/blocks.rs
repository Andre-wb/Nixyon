/// Блоки записи для нейросегментации: полуоткрытые диапазоны номеров сэмплов подряд,
/// все длиной `block`, кроме последнего. Пустой записи блоков не нужно.
pub(crate) fn blocks(total: usize, block: usize) -> Vec<(usize, usize)> {
    let block = block.max(1);
    (0..total)
        .step_by(block)
        .map(|start| (start, (start + block).min(total)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_is_cut_into_equal_blocks_and_a_shorter_tail() {
        assert_eq!(blocks(25, 10), [(0, 10), (10, 20), (20, 25)]);
    }

    #[test]
    fn record_shorter_than_a_block_is_one_block() {
        assert_eq!(blocks(7, 10), [(0, 7)]);
        assert_eq!(blocks(10, 10), [(0, 10)]);
    }

    #[test]
    fn empty_record_has_no_blocks() {
        assert!(blocks(0, 10).is_empty());
    }
}
