//! A Feeds mark applies the stored state to the loaded rows at once (spec
//! `feed-queue-item`, #1005): a marked row shows played with position zero
//! without a refetch, and rows of another feed with the same guid stay as
//! fetched.
use super::super::*;

fn feed_entry(guid: &str, feed_id: &str, position_ticks: i64) -> mbv_queue::FeedEntry {
    mbv_queue::FeedEntry {
        guid: guid.into(),
        title: format!("Feed {guid}"),
        enclosure_url: Some(format!("https://example.test/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some(10_000_000_000),
        pub_date_secs: None,
        feed_kind: Some(mbv_queue::FeedKind::Audio),
        feed_id: Some(feed_id.into()),
        position_ticks,
        played: false,
    }
}

fn rows(entries: &[mbv_queue::FeedEntry]) -> Vec<(bool, i64)> {
    entries
        .iter()
        .map(|entry| (entry.played, entry.position_ticks))
        .collect()
}

#[test]
fn mark_played_shows_played_with_position_zero_in_the_loaded_rows() {
    let mut app = make_app_stub();
    let marked = feed_entry("ep-1", "feed-a", 4_000_000_000);
    let other_feed_same_guid = feed_entry("ep-1", "feed-b", 7_000_000_000);
    let other_entry = feed_entry("ep-2", "feed-a", 5_000_000_000);
    app.feed_tab.entries = vec![vec![marked.clone(), other_entry.clone()]];
    app.feed_tab.all_entries = vec![
        marked.clone(),
        other_feed_same_guid.clone(),
        other_entry.clone(),
    ];

    app.execute_context_action(Some(ContextAction::FeedsMarkPlayed(vec![marked])), None);

    assert_eq!(
        rows(&app.feed_tab.entries[0]),
        vec![(true, 0), (false, 5_000_000_000)]
    );
    assert_eq!(
        rows(&app.feed_tab.all_entries),
        vec![(true, 0), (false, 7_000_000_000), (false, 5_000_000_000)]
    );
    assert_eq!(
        app.feed_entry_state
            .get("", "feed-a", "ep-1")
            .map(|state| { (state.played, state.position_ticks) }),
        Some((true, 0))
    );
    assert!(!app.feed_tab.loading, "a mark must not start a refetch");
    assert_eq!(app.feed_tab.pending_results, 0);
}
