//! Feeds context menus follow the standard action set (context-menu spec,
//! "Library lists share one standard action set" and "The played-state entry
//! is labelled Played and follows state"). Standard-media-context-menus
//! task 4.2.
use super::super::*;
use rstest::rstest;

fn feed_entry(guid: &str, played: bool) -> mbv_queue::FeedEntry {
    mbv_queue::FeedEntry {
        guid: guid.into(),
        title: format!("Feed {guid}"),
        enclosure_url: Some(format!("https://example.test/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(mbv_queue::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played,
    }
}

/// Open the Feeds menu for `entries` and return the ordered entry labels.
fn feeds_menu_labels(app: &mut App, entries: Vec<mbv_queue::FeedEntry>) -> Vec<&'static str> {
    app.open_feeds_context_menu(entries, None);
    match app.pending_overlay.take() {
        Some(mbv_ui_model::overlay::OverlayRequest::ContextMenu(menu)) => {
            menu.entries.iter().map(|entry| entry.label).collect()
        }
        other => panic!("expected a context menu overlay, got {other:?}"),
    }
}

/// One case per entry shape: the standard action set for that shape, with the
/// single-row mark entry chosen from the entry's played state.
#[rstest]
#[case::unplayed_single_entry(
    vec![feed_entry("a", false)],
    &["Play", "Add to Queue", "Mark Played"],
)]
#[case::played_single_entry(
    vec![feed_entry("a", true)],
    &["Play", "Add to Queue", "Mark Unplayed"],
)]
#[case::two_entries(
    vec![feed_entry("a", false), feed_entry("b", true)],
    &["Play", "Shuffle", "Add to Queue", "Mark Played", "Mark Unplayed"],
)]
fn feeds_menu_follows_the_standard_action_set_for(
    #[case] entries: Vec<mbv_queue::FeedEntry>,
    #[case] expected: &[&str],
) {
    let mut app = make_app_stub();
    assert_eq!(feeds_menu_labels(&mut app, entries), expected);
}
