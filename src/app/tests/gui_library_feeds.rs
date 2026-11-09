//! Tests of the GUI Library's Feeds data path (add-gui-library-panel design
//! D12, row 11.1): the merged Latest list and a subscription's scoped list,
//! the presented Feeds tab and a saved Feeds location, and the pill and pick
//! commands' store and stale arms.
//!
//! The fetch boundary is the App's own result channel, so a test owns every
//! entry it asserts and no request leaves the process.

use super::*;
use mbv_config::{GUI_LAUNCH_STATE_VERSION, GuiLaunchState, LaunchPanelFocus, TabIdentity};

/// The two subscriptions the tests configure: the feed identity is the URL,
/// as the fetch stamps it on every entry.
const FEED_A: &str = "https://example.test/a";
const FEED_B: &str = "https://example.test/b";

/// A stub `App` whose two subscriptions' fetches have landed, with the feed
/// tab active: `a-old` (100) and `a-new` (300) for the first, `b-mid` (200)
/// and `b-undated` for the second.
fn fetched_feeds_app() -> App {
    let mut app = make_app_stub();
    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A), ("Feed B", FEED_B)]);
    app.tab = TabSelection::Feeds;
    app.send_gui_feed_fetch_result_for_test(
        0,
        &[
            ("a-old", Some(100)),
            ("a-new", Some(300)),
            ("a-undated", None),
        ],
    );
    app.send_gui_feed_fetch_result_for_test(1, &[("b-mid", Some(200)), ("b-undated", None)]);
    assert!(app.drain_feed_tab_results(), "both results drained");
    app
}

/// The entry guids the App's selected pill lists, in order.
fn listed_guids(app: &App) -> Vec<String> {
    app.gui_feeds_entries()
        .iter()
        .map(|entry| entry.guid.clone())
        .collect()
}

fn feed_b_pill() -> crate::app::gui_library::GuiFeedPill {
    crate::app::gui_library::GuiFeedPill::Subscription(FEED_B.into())
}

/// Latest merges both subscriptions' entries newest first, with the undated
/// entries last (row 11.1, contract a).
#[test]
fn latest_lists_both_subscriptions_newest_first_with_undated_last() {
    let app = fetched_feeds_app();

    assert_eq!(
        listed_guids(&app),
        ["a-new", "b-mid", "a-old", "a-undated", "b-undated"],
        "Latest merges both subscriptions newest first with nothing dated last"
    );
}

/// A subscription pill scopes the list to that subscription's entries
/// (row 11.1, contract a).
#[test]
fn a_subscription_pill_lists_only_that_subscriptions_entries() {
    let mut app = fetched_feeds_app();

    app.gui_feeds_set_pill(feed_b_pill());

    assert_eq!(
        listed_guids(&app),
        ["b-mid", "b-undated"],
        "the pill's own list, in the same order"
    );
}

/// The presented destination set names the Feeds tab at the tab strip's own
/// position, labelled as the TUI labels it (row 11.1, contract b).
#[test]
fn the_presented_tabs_include_the_feeds_destination() {
    let app = fetched_feeds_app();

    let tabs = app.gui_library_tabs();

    assert_eq!(
        tabs.tabs
            .iter()
            .map(|tab| (tab.position, tab.label.as_str()))
            .collect::<Vec<_>>(),
        [(1, "Feeds")],
        "the feed tab follows every Service library, and this stub holds none"
    );
    assert_eq!(tabs.current, Some(1), "the active feed tab is current");
    assert_eq!(
        app.gui_active_destination(),
        Some(crate::app::gui_library::GuiDestinationKind::Feeds),
        "the active destination's kind is the Feeds one"
    );
}

/// A saved Feeds location restores to the feed tab and applies no saved pill
/// (row 11.1, contract b).
#[test]
fn a_saved_feeds_location_restores_to_the_feed_tab_with_no_saved_pill() {
    let mut app = fetched_feeds_app();
    app.tab = TabSelection::Home;
    let saved = GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: TabIdentity::Feeds,
        panel_focus: LaunchPanelFocus::Library,
        collections: Vec::new(),
    };

    let opened = app
        .open_gui_launch_destination(&saved)
        .expect("the feed tab resolves the saved Feeds destination");

    assert_eq!(
        app.tab,
        TabSelection::Feeds,
        "the saved Feeds destination restored"
    );
    assert!(
        app.restore_gui_launch_collections(&saved, &opened),
        "the Feeds destination needs no retry"
    );
    assert_eq!(
        app.gui_feeds_pill(),
        crate::app::gui_library::GuiFeedPill::Latest,
        "the launch file saves no pill, so the restored tab opens on Latest"
    );
}

/// A subscription pill is stored, and one the tab no longer holds changes
/// nothing (row 11.1, contract c).
#[test]
fn a_subscription_pill_is_stored_and_a_stale_one_changes_nothing() {
    let mut app = fetched_feeds_app();

    app.gui_feeds_set_pill(feed_b_pill());
    assert_eq!(app.gui_feeds_pill(), feed_b_pill(), "the pill is stored");

    app.gui_feeds_set_pill(crate::app::gui_library::GuiFeedPill::Subscription(
        "https://example.test/gone".into(),
    ));
    assert_eq!(
        app.gui_feeds_pill(),
        feed_b_pill(),
        "a subscription the tab does not hold changes nothing"
    );
}

/// The frame path's ensure falls back to Latest when the selected pill's
/// subscription leaves the tab (row 11.1, contract c).
#[test]
fn the_ensure_falls_back_to_latest_when_the_subscription_leaves() {
    let mut app = fetched_feeds_app();
    app.gui_feeds_set_pill(feed_b_pill());

    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A)]);
    app.ensure_gui_feeds_selection();

    assert_eq!(
        app.gui_feeds_pill(),
        crate::app::gui_library::GuiFeedPill::Latest,
        "a removed subscription's pill never paints"
    );
}

/// An entry pick is stored, and one the selected pill's list does not carry
/// changes nothing (row 11.1, contract c).
#[test]
fn an_entry_pick_is_stored_and_one_outside_the_selected_list_changes_nothing() {
    let mut app = fetched_feeds_app();
    app.gui_feeds_set_pill(feed_b_pill());

    app.gui_feeds_select("b-mid");
    assert_eq!(
        app.gui_feeds_picked_entry(),
        Some("b-mid"),
        "the picked entry is the select command's"
    );

    app.gui_feeds_select("a-old");
    assert_eq!(
        app.gui_feeds_picked_entry(),
        Some("b-mid"),
        "an entry the selected pill's list does not carry changes nothing"
    );
}
