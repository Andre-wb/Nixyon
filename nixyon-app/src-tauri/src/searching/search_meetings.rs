//! Поиск по всем встречам архива тем же алгоритмом, что внутри одной встречи. Встречи
//! берутся из индекса, а не из папок: удалённая встреча не находится, даже если её папка
//! ещё не стёрта. Запрос и найденный текст в лог не пишутся: это слова из разговора.

use nixyon_pipeline::{MatchKind, search};
use tauri::{AppHandle, Runtime};

use super::in_order_parallel::map_in_order;
use super::meeting_hit::MeetingHit;
use super::meeting_results::MeetingResults;
use crate::meetings::list_meetings;
use crate::meetings::transcript_file;
use crate::transcription::CommandError;
use crate::vault::FilesStore;

/// Больше совпадений интерфейс не показывает.
pub const MAX_HITS: usize = 200;

/// Встречи с каждой из меток `labels` (пустой набор: все), новые сверху; внутри встречи
/// порядок как у `search`.
pub fn find(
    store: &FilesStore,
    query: &str,
    labels: &[u32],
) -> Result<MeetingResults, CommandError> {
    let mut meetings = list_meetings::list(store)
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?;
    if query.trim().is_empty() {
        return Ok(MeetingResults {
            hits: Vec::new(),
            truncated: false,
        });
    }
    meetings.retain(|m| labels.iter().all(|l| m.labels.contains(l)));
    let per_meeting = map_in_order(&meetings, |entry| {
        let Some(transcript) = transcript_file::load(store, &entry.id) else {
            log::warn!("встреча пропущена в поиске: расшифровка не читается");
            return (0, Vec::new());
        };
        let found = search(&transcript, query);
        // Больше MAX_HITS с одной встречи всё равно не покажется.
        let hits = found
            .iter()
            .take(MAX_HITS)
            .filter_map(|h| {
                let turn = transcript.turns.iter().find(|t| t.id == h.turn_id)?;
                Some(MeetingHit {
                    meeting: entry.id.as_str().to_owned(),
                    title: entry.title.clone(),
                    turn_id: h.turn_id,
                    text: turn.text.clone(),
                    start: h.start,
                    end: h.end,
                    exact: h.kind == MatchKind::Exact,
                })
            })
            .collect::<Vec<_>>();
        (found.len(), hits)
    });
    let total: usize = per_meeting.iter().map(|(n, _)| n).sum();
    let hits = per_meeting
        .into_iter()
        .flat_map(|(_, hits)| hits)
        .take(MAX_HITS)
        .collect();
    Ok(MeetingResults {
        hits,
        truncated: total > MAX_HITS,
    })
}

#[tauri::command]
pub async fn search_meetings<R: Runtime>(
    app: AppHandle<R>,
    query: String,
    labels: Vec<u32>,
) -> Result<MeetingResults, CommandError> {
    let store = crate::vault::storage(&app).await?.store;
    tauri::async_runtime::spawn_blocking(move || find(&store, &query, &labels))
        .await
        .map_err(|e| CommandError::logged(CommandError::STORAGE_FAILED, e))?
}

#[cfg(test)]
mod tests {
    use super::super::test_archive::{add, add_generated, meeting_id};
    use super::*;
    use crate::meetings::{create_label, delete_meeting, rename_meeting, set_meeting_labels};

    fn dir(name: &str) -> FilesStore {
        crate::vault::test_store(&format!("search-meetings-{name}"))
    }

    fn places(found: &MeetingResults) -> Vec<(String, u32, bool)> {
        found
            .hits
            .iter()
            .map(|h| (h.meeting.clone(), h.turn_id, h.exact))
            .collect()
    }

    #[test]
    fn hits_from_several_meetings_come_newest_first_with_title_and_turn_text() {
        let d = dir("several");
        let old = add(&d, 1, &["Встреча завтра в десять."]);
        let new = add(&d, 2, &["Про бюджет.", "Новая встреча по проекту"]);
        add(&d, 3, &["Здесь ничего"]);
        rename_meeting::rename(&d, &new, "Планёрка").unwrap();
        let found = find(&d, "встреча", &[]).unwrap();
        assert_eq!(
            places(&found),
            [(new.clone(), 2, true), (old.clone(), 1, true)]
        );
        assert!(!found.truncated);
        let hit = &found.hits[0];
        assert_eq!(hit.title.as_deref(), Some("Планёрка"));
        assert_eq!(hit.text, "Новая встреча по проекту");
        assert_eq!((hit.start, hit.end), (6, 13));
        assert_eq!(found.hits[1].title, None);
        let json = serde_json::to_value(hit).unwrap();
        assert!(json["turnId"].is_number());
    }

    #[test]
    fn typo_finds_the_word_as_a_fuzzy_hit() {
        let d = dir("typo");
        let id = add(&d, 1, &["Встреча перенесена"]);
        assert_eq!(places(&find(&d, "встерча", &[]).unwrap()), [(id, 1, false)]);
    }

    #[test]
    fn yo_and_ye_are_the_same_letter_both_ways() {
        let d = dir("yo");
        let a = add(&d, 1, &["Ёлка в холле"]);
        let b = add(&d, 2, &["Елка у входа"]);
        for query in ["елка", "ёлка"] {
            assert_eq!(
                places(&find(&d, query, &[]).unwrap()),
                [(b.clone(), 1, true), (a.clone(), 1, true)],
                "{query}"
            );
        }
    }

    #[test]
    fn deleted_meeting_is_not_found_even_if_its_folder_stays() {
        let d = dir("deleted");
        let kept = add(&d, 1, &["встреча раз"]);
        let gone = add(&d, 2, &["встреча два"]);
        delete_meeting::delete(&d, &gone, &|_| Err(std::io::Error::other("не стёрлась"))).unwrap();
        assert_eq!(
            places(&find(&d, "встреча", &[]).unwrap()),
            [(kept, 1, true)]
        );
    }

    #[test]
    fn label_filter_keeps_meetings_with_every_chosen_label() {
        let d = dir("labels");
        let both = add(&d, 1, &["встреча"]);
        let one = add(&d, 2, &["встреча"]);
        add(&d, 3, &["встреча"]);
        let a = create_label::create(&d, "Отдел").unwrap().id;
        let b = create_label::create(&d, "Склад").unwrap().id;
        set_meeting_labels::set(&d, &both, &[a, b]).unwrap();
        set_meeting_labels::set(&d, &one, &[a]).unwrap();
        let ids = |labels: &[u32]| -> Vec<String> {
            find(&d, "встреча", labels)
                .unwrap()
                .hits
                .into_iter()
                .map(|h| h.meeting)
                .collect()
        };
        assert_eq!(ids(&[a]), [one.clone(), both.clone()]);
        assert_eq!(ids(&[a, b]), std::slice::from_ref(&both));
        assert_eq!(ids(&[]).len(), 3);
        assert_eq!(ids(&[99]), Vec::<String>::new());
    }

    #[test]
    fn hits_over_the_limit_are_cut_and_marked() {
        let d = dir("limit");
        let many = vec!["да"; MAX_HITS - 1];
        add(&d, 1, &many);
        add(&d, 2, &["да, да"]);
        let found = find(&d, "да", &[]).unwrap();
        assert_eq!(found.hits.len(), MAX_HITS);
        assert!(found.truncated);
        assert_eq!(found.hits[0].meeting, meeting_id(2));
        let d2 = dir("limit-exact");
        add(&d2, 1, &vec!["да"; MAX_HITS]);
        let found = find(&d2, "да", &[]).unwrap();
        assert_eq!(found.hits.len(), MAX_HITS);
        assert!(!found.truncated);
    }

    #[test]
    fn empty_query_finds_nothing() {
        let d = dir("empty");
        add(&d, 1, &["встреча"]);
        for query in ["", "   "] {
            let found = find(&d, query, &[]).unwrap();
            assert!(found.hits.is_empty() && !found.truncated);
        }
    }

    #[test]
    fn unreadable_transcript_is_skipped_and_broken_index_is_an_error() {
        let d = dir("unreadable");
        let bad = add(&d, 1, &["встреча"]);
        let good = add(&d, 2, &["встреча"]);
        let folder = bad.replace([':', '-'], "");
        std::fs::write(
            d.dir()
                .join("meetings")
                .join(folder)
                .join("transcript.json"),
            b"{",
        )
        .unwrap();
        assert_eq!(
            places(&find(&d, "встреча", &[]).unwrap()),
            [(good, 1, true)]
        );
        std::fs::write(d.dir().join(crate::meetings::INDEX_FILE), "{").unwrap();
        assert_eq!(
            find(&d, "встреча", &[]).unwrap_err().code,
            CommandError::STORAGE_FAILED
        );
    }

    #[test]
    fn fifty_meetings_are_searched_newest_first() {
        let d = dir("fifty");
        for n in 0..50 {
            add_generated(&d, n, 20);
        }
        let found = find(&d, "обсудим сроки", &[]).unwrap();
        assert!(!found.hits.is_empty());
        let order: Vec<_> = found.hits.iter().map(|h| h.meeting.clone()).collect();
        let mut sorted = order.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(order, sorted);
        assert!(found.hits.iter().all(|h| h.text[..].contains("обсудим")));
    }
}
