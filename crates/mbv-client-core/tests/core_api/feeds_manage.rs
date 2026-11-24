use mbv_client_core::make_app_stub;
use mbv_config::FeedSubscription;
use mbv_queue::FeedKind;
use mbv_ui_model::feeds_manage::{FeedAddDrain, FeedAddResult, FeedsManagePopup};
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

fn sub(name: &str, url: &str, kind: FeedKind) -> FeedSubscription {
    FeedSubscription {
        name: name.to_string(),
        url: url.to_string(),
        kind,
    }
}

/// A stub app holding the given configured subscriptions, with the Feeds tab
/// they project.
fn app_with_feeds(feeds: Vec<FeedSubscription>) -> mbv_client_core::App {
    let mut app = make_app_stub();
    app.config_for_test(|config| config.feeds = feeds);
    app.sync_feed_subscriptions();
    app
}

/// A stub app holding the given Feeds management popup, as the overlay mount
/// leaves it.
fn app_with_feed_popup(popup: FeedsManagePopup) -> mbv_client_core::App {
    let mut app = make_app_stub();
    app.seed_feeds_manage_popup_for_test(popup);
    app
}

/// A popup whose newest queued add result is `id`, with `pending` as the
/// submission the popup is waiting on.
fn popup_waiting_on(pending: u64, result: FeedAddResult) -> FeedsManagePopup {
    let mut popup = FeedsManagePopup::new();
    popup
        .add_tx
        .send(result)
        .expect("the popup's own channel is open");
    popup.pending_add = Some(pending);
    popup
}

fn enter() -> KeyEvent {
    KeyEvent::new(Key::Enter, KeyModifiers::NONE)
}

#[test]
fn feed_state_transition_is_safe_without_emby() {
    let mut app = app_with_feeds(vec![sub(
        "Feed-only",
        "https://example.test/feed.xml",
        FeedKind::Audio,
    )]);

    assert!(!app.emby_client_present_for_test());
    app.sync_feed_subscriptions();
    assert_eq!(app.feeds_state().subscriptions[0].name, "Feed-only");
}

/// §6.3: editing a subscription changes only its name and kind. A URL
/// typed into the (read-only, in real input handling) form field must not
/// reach the persisted subscription -- the original URL is always kept.
/// §6.3: remove rewrites `config.feeds` without the removed entry, leaving
/// the others in order.
#[test]
fn remove_feed_confirmed_rewrites_list() {
    let mut app = app_with_feeds(vec![
        sub("A", "https://a", FeedKind::Video),
        sub("B", "https://b", FeedKind::Video),
        sub("C", "https://c", FeedKind::Video),
    ]);

    app.request_feed_removal("https://b");
    let prompt = app
        .take_pending_overlay_request_for_test()
        .expect("the removal raises its confirmation prompt");
    let mbv_ui_model::overlay::OverlayRequest::Confirm(modal) = prompt else {
        panic!("the removal confirms through a modal");
    };
    app.apply_confirm_action(modal.on_confirm, enter());

    let names: Vec<String> = app
        .configured_feeds()
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
    let popup = popup_waiting_on(
        5,
        FeedAddResult {
            id: 3,
            name: "Stale".into(),
            url: "https://stale".into(),
            kind: FeedKind::Video,
            result: Ok(()),
        },
    );
    let mut app = app_with_feed_popup(popup);

    let drain = app.drain_feed_add_results();

    assert!(
        matches!(drain, FeedAddDrain::Discarded),
        "the stale message should still be drained"
    );
    assert!(app.configured_feeds().is_empty());
    assert_eq!(
        app.feeds_manage_pending_add_for_test(),
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
    let popup = popup_waiting_on(
        1,
        FeedAddResult {
            id: 1,
            name: "Broken".into(),
            url: "https://broken".into(),
            kind: FeedKind::Video,
            result: Err(mbv_ui_model::UiModelError::operation("connection refused")),
        },
    );
    let mut app = app_with_feed_popup(popup);

    app.drain_feed_add_results();

    assert!(app.configured_feeds().is_empty());
    assert!(app.newest_toast_text().contains("Couldn't add feed"));
}

/// D5 of `add-gui-settings-screen`: the URL is the subscription list's
/// only stable identity (edit and remove resolve by first URL match), so
/// an add whose URL is already subscribed is refused and the list is
/// unchanged. §6.2: the refusal surfaces through the same failed-add
/// drain as a fetch failure ("Couldn't add feed: ...").
#[test]
fn duplicate_url_add_is_refused_and_list_unchanged() {
    let mut app = app_with_feeds(vec![sub("A", "https://a", FeedKind::Video)]);
    app.seed_feeds_manage_popup_for_test(FeedsManagePopup::new());

    let id = app.begin_feed_add("B", "https://a", FeedKind::Video);

    assert!(
        id.is_some(),
        "a duplicate add must report through the add-result drain"
    );
    assert!(
        matches!(app.drain_feed_add_results(), FeedAddDrain::Failed),
        "the refusal drains as a handled result"
    );
    let feeds = app.configured_feeds();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].name, "A");
    assert_eq!(feeds[0].url, "https://a");
    assert!(app.newest_toast_text().contains("Couldn't add feed"));
    assert_eq!(app.feeds_manage_pending_add_for_test(), None);
}
