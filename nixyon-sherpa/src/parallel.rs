/// Сколько работы одной записи идёт параллельно: эмбеддингов голоса и распознаваний
/// кусков речи. На каждый параллельный поток своя копия модели.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parallel {
    pub embed_jobs: usize,
    pub asr_jobs: usize,
}

impl Default for Parallel {
    fn default() -> Self {
        Self {
            embed_jobs: 1,
            asr_jobs: 1,
        }
    }
}
