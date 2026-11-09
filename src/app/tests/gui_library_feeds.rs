//! Tests of the GUI Library's Feeds data path (add-gui-library-panel design
//! D12, row 11.1): one background fetch's completion lands its subscription's
//! entries on the App through the tick's own drain, Latest merges every
//! subscription newest first with undated entries last, a subscription pill
//! scopes the list to that subscription, the presented destination set names
//! the Feeds tab and a saved Feeds location restores to it with no pill
//! persistence, and the pill and pick commands store their selection while
//! ignoring one the subscription set or the selected list no longer holds.
//!
//! The fetch boundary is the App's own result channel, so no request leaves
//! the process and no worker outlives the assertion.

use super::*;
use mbv_config::{GUI_LAUNCH_STATE_VERSION, GuiLaunchState, LaunchPanelFocus, TabIdentity};

/// The two subscriptions the tests configure: the feed identity is the URL,
/// as the fetch stamps it on every entry.
const FEED_A: &str = "https://example.test/a";
const FEED_B: &str = "https://example.test/b";

/// A stub `App` holding the two subscriptions with no fetched entries: the
/// state the construction's subscription copy leaves before any fetch lands.
fn feeds_app() -> App {
    let mut app = make_app_stub();
    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A), ("Feed B", FEED_B)]);
    app.tab = TabSelection::Feeds;
    app
}

/// The entry guids the App's selected pill lists, in order.
fn listed_guids(app: &App) -> Vec<String> {
    app.gui_feeds_entries()
        .iter()
        .map(|entry| entry.guid.clone())
        .collect()
}

/// The merged Latest list holds both subscriptions' entries newest first with
/// the undated entry last, and a subscription pill scopes it to that one
/// subscription's list (row 11.1, contract a): the drain rebuilds the merged
/// order from the per-subscription lists, and the pill switch reads the
/// per-subscription list alone.
#[test]
fn latest_merges_both_subscriptions_and_a_subscription_pill_scopes_the_list() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = feeds_app();
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

    assert_eq!(
        listed_guids(&app),
        ["a-new", "b-mid", "a-old", "a-undated", "b-undated"],
        "Latest merges both subscriptions newest first with nothing dated last"
    );

    app.gui_feeds_set_pill(crate::app::gui_library::GuiFeedPill::Subscription(
        FEED_B.into(),
    ));
    assert_eq!(
        listed_guids(&app),
        ["b-mid", "b-undated"],
        "a subscription pill lists only that subscription's entries"
    );
}

/// The presented destination set names the Feeds tab at the tab strip's own
/// position, and a saved Feeds location restores to it with no saved pill
/// (row 11.1, contract b): the tab follows every Service library, the active
/// Feeds tab is current, and the restore applies no pill because the launch
/// file keeps none.
#[test]
fn the_presented_tabs_include_feeds_and_a_saved_location_restores_it() {
    let mut app = feeds_app();

    let tabs = app.gui_library_tabs();
    let labels: Vec<&str> = tabs.tabs.iter().map(|tab| tab.label.as_str()).collect();
    assert_eq!(labels, ["Feeds"], "the feed tab presents itself");
    let position = 1 + app.libs.len() + app.audiobookshelf_libraries.len();
    assert_eq!(
        tabs.tabs.iter().map(|tab| tab.position).collect::<Vec<_>>(),
        [position],
        "the Feeds tab follows every Service library"
    );
    assert_eq!(
        tabs.current,
        Some(position),
        "the active Feeds tab is current"
    );
    assert_eq!(
        app.gui_active_destination(),
        Some(crate::app::gui_library::GuiDestinationKind::Feeds),
        "the active destination's kind is the Feeds one"
    );

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
        "the Feeds destination saves no pill and needs no retry"
    );
    assert_eq!(
        app.gui_feeds_pill(),
        crate::app::gui_library::GuiFeedPill::Latest,
        "the restored destination keeps the default Latest pill"
    );
}

/// The pill and pick commands store their selection and ignore one the
/// published data no longer holds (row 11.1, contract c): a stored
/// subscription pill survives a later stale one, a stale pill falls back to
/// Latest when its subscription leaves the tab, and an entry the selected
/// pill's list does not carry changes nothing.
#[test]
fn the_pill_and_pick_commands_store_their_selection_and_ignore_a_stale_one() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = feeds_app();
    app.send_gui_feed_fetch_result_for_test(1, &[("b-mid", Some(200))]);
    assert!(app.drain_feed_tab_results(), "the result drained");

    let feed_b = crate::app::gui_library::GuiFeedPill::Subscription(FEED_B.into());
    app.gui_feeds_set_pill(feed_b.clone());
    assert_eq!(app.gui_feeds_pill(), feed_b, "the pill is stored");

    app.gui_feeds_select("b-mid");
    assert_eq!(
        app.gui_feeds_picked_entry(),
        Some("b-mid"),
        "the picked entry is the select command's"
    );
    app.gui_feeds_select("gone");
    assert_eq!(
        app.gui_feeds_picked_entry(),
        Some("b-mid"),
        "an entry the selected pill's list does not carry changes nothing"
    );

    app.gui_feeds_set_pill(crate::app::gui_library::GuiFeedPill::Subscription(
        "https://example.test/gone".into(),
    ));
    assert_eq!(
        app.gui_feeds_pill(),
        feed_b,
        "a stale subscription pill changes nothing"
    );

    // The subscription leaves the tab: the ensure's one write converges the
    // store on the set the tab holds, so the dead pill never paints.
    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A)]);
    app.ensure_gui_feeds_selection();
    assert_eq!(
        app.gui_feeds_pill(),
        crate::app::gui_library::GuiFeedPill::Latest,
        "a removed subscription's pill falls back to Latest"
    );
}
