use crate::app::state::types::feeds_manage::{FeedAddResult, FeedsManagePopup};
use crate::app::tests::make_app_stub;
use crate::app::Model;
use mbv_core::config::{FeedKind, FeedSubscription};

fn sub(name: &str, url: &str, kind: FeedKind) -> FeedSubscription {
    FeedSubscription {
        name: name.to_string(),
        url: url.to_string(),
        kind,
    }
}

#[test]
fn feed_state_transition_is_safe_without_emby() {
    let mut app = make_app_stub();
    assert!(app.emby_runtime.client.is_none());
    app.config.lock().unwrap().feeds = vec![sub(
        "Feed-only",
        "https://example.test/feed.xml",
        FeedKind::Audio,
    )];
    app.sync_feed_subscriptions();
    assert_eq!(app.feed_tab.subscriptions[0].name, "Feed-only");
}

/// §6.3: editing a subscription changes only its name and kind. A URL
/// typed into the (read-only, in real input handling) form field must not
/// reach the persisted subscription -- the original URL is always kept.
/// §6.3: remove rewrites `config.feeds` without the removed entry, leaving
/// the others in order.
#[test]
fn remove_feed_confirmed_rewrites_list() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().feeds = vec![
        sub("A", "https://a", FeedKind::Video),
        sub("B", "https://b", FeedKind::Video),
        sub("C", "https://c", FeedKind::Video),
    ];

    app.remove_feed_confirmed(1);

    let names: Vec<String> = app
        .config
        .lock()
        .unwrap()
        .feeds
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(names, vec!["A", "C"]);
}

/// §6.4: removing the last subscription while the Feeds tab is selected
/// falls back to Home.
/// §6.4: after any mutation, shell-owned Feed entries are cleared for the new
/// (possibly shorter) subscription list -- no auto-fetch.
/// §6.2: a background add result whose id no longer matches the popup's
/// current `pending_add` -- superseded by a later submission -- is
/// dropped without touching config.
#[test]
fn stale_add_result_is_dropped() {
    let mut model = Model::new(make_app_stub());
    let mut popup = FeedsManagePopup::new();
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
    popup.pending_add = Some(5); // a newer submission is the current one
    model.feeds_manage = Some(popup);

    let had_events = model.drain_feed_add_results();

    assert!(had_events, "the stale message should still be drained");
    assert!(model.app.config.lock().unwrap().feeds.is_empty());
    assert_eq!(
        model.feeds_manage.as_ref().unwrap().pending_add,
        Some(5),
        "the still-current pending id must be untouched"
    );
}

/// §6.2: cancelling an in-flight add (Esc) clears `pending_add`; the
/// fetch's eventual result must then be dropped as stale.
/// §6.2/§6.3: a matching add result appends to `config.feeds` and returns
/// the overlay to the List stage.
/// §6.2: a fetch failure surfaces via the status/flash path and does not
/// save.
#[test]
fn add_fetch_failure_does_not_save() {
    let mut model = Model::new(make_app_stub());
    let mut popup = FeedsManagePopup::new();
    popup.pending_add = Some(1);
    popup
        .add_tx
        .send(FeedAddResult {
            id: 1,
            name: "Broken".into(),
            url: "https://broken".into(),
            kind: FeedKind::Video,
            result: Err("connection refused".into()),
        })
        .unwrap();
    model.feeds_manage = Some(popup);

    model.drain_feed_add_results();

    assert!(model.app.config.lock().unwrap().feeds.is_empty());
    assert!(model.app.status.contains("Couldn't add feed"));
}
