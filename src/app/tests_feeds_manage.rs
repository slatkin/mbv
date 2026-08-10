use super::types_feeds_manage::{
    FeedAddResult, FeedForm, FeedFormField, FeedsManagePopup, FeedsManageStage,
};
use super::*;
use crate::app::tests::make_app_stub;
use mbv_core::config::{FeedKind, FeedSubscription};

fn sub(name: &str, url: &str, kind: FeedKind) -> FeedSubscription {
    FeedSubscription {
        name: name.to_string(),
        url: url.to_string(),
        kind,
    }
}

/// §6.3: editing a subscription changes only its name and kind. A URL
/// typed into the (read-only, in real input handling) form field must not
/// reach the persisted subscription -- the original URL is always kept.
#[test]
fn edit_changes_name_and_kind_but_not_url() {
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.feeds = vec![sub(
        "Old Name",
        "https://example.test/original",
        FeedKind::Video,
    )];
    app.feeds_manage_popup = Some(FeedsManagePopup::new());
    let mut form = FeedForm::new_edit(0, &app.client.lock().unwrap().config.feeds[0].clone());
    form.name = "New Name".to_string();
    form.url = "https://example.test/attempted-change".to_string();
    form.kind = FeedKind::Audio;
    form.focus = FeedFormField::Name;
    app.feeds_manage_popup.as_mut().unwrap().stage = FeedsManageStage::Form(form);

    app.submit_feed_form();

    let feeds = app.client.lock().unwrap().config.feeds.clone();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].name, "New Name");
    assert_eq!(feeds[0].kind, FeedKind::Audio);
    assert_eq!(
        feeds[0].url, "https://example.test/original",
        "edit must not change the URL"
    );
}

/// §6.3: remove rewrites `config.feeds` without the removed entry, leaving
/// the others in order.
#[test]
fn remove_feed_confirmed_rewrites_list() {
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.feeds = vec![
        sub("A", "https://a", FeedKind::Video),
        sub("B", "https://b", FeedKind::Video),
        sub("C", "https://c", FeedKind::Video),
    ];

    app.remove_feed_confirmed(1);

    let names: Vec<String> = app
        .client
        .lock()
        .unwrap()
        .config
        .feeds
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(names, vec!["A", "C"]);
}

/// §6.4: removing the last subscription while the Feeds tab is selected
/// falls back to Home.
#[test]
fn removing_last_subscription_falls_back_to_home() {
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.feeds = vec![sub("Only", "https://only", FeedKind::Video)];
    app.sync_feed_subscriptions();
    app.tab = TabSelection::Feeds;

    app.remove_feed_confirmed(0);

    assert!(app.client.lock().unwrap().config.feeds.is_empty());
    assert!(
        app.tab.is_home(),
        "expected fallback to Home, got {:?}",
        app.tab
    );
}

/// §6.4: after any mutation, `feed_tab` entries are cleared and
/// `selected_group`/cursor/scroll are clamped into range for the new
/// (possibly shorter) subscription list -- no auto-fetch.
#[test]
fn post_mutation_clears_entries_and_clamps_group_and_cursor() {
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.feeds = vec![
        sub("A", "https://a", FeedKind::Video),
        sub("B", "https://b", FeedKind::Video),
    ];
    app.sync_feed_subscriptions();
    app.feed_tab.entries = vec![
        vec![mbv_core::playback_queue::FeedEntry {
            guid: "a1".into(),
            title: "A1".into(),
            enclosure_url: None,
            link: Some("https://a/1".into()),
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: Some(mbv_core::config::FeedKind::Video),
            feed_id: None,
            position_ticks: 0,
            played: false,
        }],
        vec![mbv_core::playback_queue::FeedEntry {
            guid: "b1".into(),
            title: "B1".into(),
            enclosure_url: None,
            link: Some("https://b/1".into()),
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: Some(mbv_core::config::FeedKind::Video),
            feed_id: None,
            position_ticks: 0,
            played: false,
        }],
    ];
    app.feed_tab.rebuild_all_entries();
    app.feed_tab.selected_group = 2; // subscription B
    app.feed_tab.cursor = 0;

    // Remove subscription B (index 1): group 2 no longer exists.
    app.remove_feed_confirmed(1);

    assert_eq!(app.feed_tab.subscriptions.len(), 1);
    assert!(
        app.feed_tab.entries.iter().all(|e| e.is_empty()),
        "fetched entries must be cleared, not carried over"
    );
    assert!(app.feed_tab.all_entries.is_empty());
    assert!(
        app.feed_tab.selected_group < app.feed_tab.group_count(),
        "selected_group must be clamped into range"
    );
    assert_eq!(app.feed_tab.cursor, 0);
}

/// §6.2: a background add result whose id no longer matches the popup's
/// current `pending_add` -- superseded by a later submission -- is
/// dropped without touching config.
#[test]
fn stale_add_result_is_dropped() {
    let mut app = make_app_stub();
    let popup = FeedsManagePopup::new();
    popup
        .add_tx
        .send(FeedAddResult {
            id: 3,
            name: "Stale".into(),
            url: "https://stale".into(),
            kind: FeedKind::Video,
            result: Ok(()),
        })
        .unwrap();
    let mut popup = popup;
    popup.pending_add = Some(5); // a newer submission is the current one
    app.feeds_manage_popup = Some(popup);

    let had_events = app.drain_feed_add_results();

    assert!(had_events, "the stale message should still be drained");
    assert!(app.client.lock().unwrap().config.feeds.is_empty());
    assert_eq!(
        app.feeds_manage_popup.as_ref().unwrap().pending_add,
        Some(5),
        "the still-current pending id must be untouched"
    );
}

/// §6.2: cancelling an in-flight add (Esc) clears `pending_add`; the
/// fetch's eventual result must then be dropped as stale.
#[test]
fn cancelled_add_result_is_dropped() {
    let mut app = make_app_stub();
    app.feeds_manage_popup = Some(FeedsManagePopup::new());
    app.feeds_manage_popup.as_mut().unwrap().pending_add = Some(1);
    app.feeds_manage_popup
        .as_ref()
        .unwrap()
        .add_tx
        .send(FeedAddResult {
            id: 1,
            name: "Cancelled".into(),
            url: "https://cancelled".into(),
            kind: FeedKind::Video,
            result: Ok(()),
        })
        .unwrap();

    // Esc while the add is in flight.
    app.cancel_feed_form();

    let had_events = app.drain_feed_add_results();

    assert!(had_events);
    assert!(app.client.lock().unwrap().config.feeds.is_empty());
}

/// §6.2/§6.3: a matching add result appends to `config.feeds` and returns
/// the overlay to the List stage.
#[test]
fn matching_add_result_appends_feed_and_returns_to_list() {
    let mut app = make_app_stub();
    app.feeds_manage_popup = Some(FeedsManagePopup::new());
    app.feeds_manage_popup.as_mut().unwrap().pending_add = Some(7);
    app.feeds_manage_popup.as_mut().unwrap().stage = FeedsManageStage::Form(FeedForm::new_add());
    app.feeds_manage_popup
        .as_ref()
        .unwrap()
        .add_tx
        .send(FeedAddResult {
            id: 7,
            name: "New Feed".into(),
            url: "https://new".into(),
            kind: FeedKind::Audio,
            result: Ok(()),
        })
        .unwrap();

    app.drain_feed_add_results();

    let feeds = app.client.lock().unwrap().config.feeds.clone();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].name, "New Feed");
    assert!(matches!(
        app.feeds_manage_popup.as_ref().unwrap().stage,
        FeedsManageStage::List
    ));
    assert_eq!(app.feeds_manage_popup.as_ref().unwrap().pending_add, None);
}

/// §6.2: a fetch failure surfaces via the status/flash path and does not
/// save.
#[test]
fn add_fetch_failure_does_not_save() {
    let mut app = make_app_stub();
    app.feeds_manage_popup = Some(FeedsManagePopup::new());
    app.feeds_manage_popup.as_mut().unwrap().pending_add = Some(1);
    app.feeds_manage_popup
        .as_ref()
        .unwrap()
        .add_tx
        .send(FeedAddResult {
            id: 1,
            name: "Broken".into(),
            url: "https://broken".into(),
            kind: FeedKind::Video,
            result: Err("connection refused".into()),
        })
        .unwrap();

    app.drain_feed_add_results();

    assert!(app.client.lock().unwrap().config.feeds.is_empty());
    assert!(app.status.contains("Couldn't add feed"));
}
