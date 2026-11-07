//! Замер поиска по архиву: 50 встреч по часу (около 150 слов в минуту, 600 реплик по 15
//! слов). Время не проверяется, только печатается: на CI оно плавает.

use std::time::Instant;

use super::search_meetings::find;
use super::test_archive::add_generated;

#[test]
#[ignore = "замер, печатает время"]
fn search_over_fifty_hour_long_meetings_timing() {
    let store = crate::vault::test_store("search-timing");
    for n in 0..50 {
        add_generated(&store, n, 600);
    }
    for query in ["встреча", "встерча", "елка", "обсудим сроки", "да"]
    {
        let started = Instant::now();
        let found = find(&store, query, &[]).unwrap();
        println!(
            "запрос {query:?}: {:?}, показано {}, обрезано {}",
            started.elapsed(),
            found.hits.len(),
            found.truncated
        );
        assert!(!found.hits.is_empty());
    }
}
