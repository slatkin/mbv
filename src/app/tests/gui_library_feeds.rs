//! Tests of the GUI Library's Feeds data path (add-gui-library-panel design
//! D12, row 11.1): the merged All list and a checked subscription's list, two
//! checked subscriptions, the Oldest order with an undated entry last, the
//! presented Feeds tab and a saved Feeds location, and the collection and pick
//! commands' store and stale arms.
//!
//! The fetch boundary is the App's own result channel, so a test owns every
//! entry it asserts and no request leaves the process.

use super::*;
use mbv_config::{GUI_LAUNCH_STATE_VERSION, GuiLaunchState, TabIdentity};
use mbv_feed::FeedPlatform;
use mbv_ui_model::media_list::MediaSemanticState;
use rstest::rstest;

/// The two subscriptions the tests configure: the feed identity is the URL,
/// as the fetch stamps it on every entry.
const FEED_A: &str = "https://example.test/a";
const FEED_B: &str = "https://example.test/b";

/// A `YouTube` channel feed and a generic podcast feed, the two subscription
/// URLs the Feed-mark tests check.
const YOUTUBE_FEED: &str = "https://www.youtube.com/feeds/videos.xml?channel_id=UCabc";
const YOUTUBE_FEED_B: &str = "https://www.youtube.com/feeds/videos.xml?channel_id=UCdef";
const GENERIC_FEED: &str = "https://example.test/generic";

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

/// A stub `App` holding one `YouTube` subscription and one generic podcast
/// subscription, with the Feeds tab active and nothing checked, so a test owns
/// every check it makes.
fn platform_feeds_app() -> App {
    let mut app = make_app_stub();
    app.seed_gui_feeds_for_test(&[("YouTube", YOUTUBE_FEED), ("Podcast", GENERIC_FEED)]);
    app.tab = TabSelection::Feeds;
    app
}

/// The Feed platform the checked subscriptions share (gui-library-header-
/// service-marks design D2, D6; the `gui-client` spec's contract "the Feeds
/// mark follows the checked subscriptions"): one platform's checked
/// subscriptions share its mark, and a checked generic subscription leaves
/// none.
#[rstest]
#[case::only_youtube_checked(&[YOUTUBE_FEED], Some(FeedPlatform::YouTube))]
#[case::youtube_and_generic_checked(&[YOUTUBE_FEED, GENERIC_FEED], None)]
fn the_feeds_platform_follows_the_checked_subscriptions(
    #[case] checked: &[&str],
    #[case] expected: Option<FeedPlatform>,
) {
    let mut app = platform_feeds_app();

    for subscription in checked {
        toggle_subscription(&mut app, subscription);
    }

    assert_eq!(
        gui_feeds_platform(&app.gui_feeds_projection().collections),
        expected,
        "the checked subscriptions decide the Feed platform"
    );
}

/// All over `YouTube`-only subscriptions shares no Feed platform (design D6; the
/// contract "the Feeds mark follows the checked subscriptions"): the merged
/// view is not one selection of one platform, so the header keeps the RSS mark.
#[test]
fn all_over_youtube_only_subscriptions_shares_no_feeds_platform() {
    let mut app = make_app_stub();
    app.seed_gui_feeds_for_test(&[("YouTube A", YOUTUBE_FEED), ("YouTube B", YOUTUBE_FEED_B)]);
    app.tab = TabSelection::Feeds;

    let collections = app.gui_feeds_projection().collections;

    assert!(
        collections.all,
        "the selection starts on All, so no subscription is checked"
    );
    assert_eq!(gui_feeds_platform(&collections), None);
}

/// Press one subscription in the Feeds destination's collection dropdown.
fn toggle_subscription(app: &mut App, subscription: &str) {
    app.apply_gui_library_command(GuiLibraryCommand::CollectionToggle {
        target: GuiCollectionTarget::Feeds,
        key: subscription.to_string(),
    });
}

/// The entry guids the App's selected pill lists, in order.
fn listed_guids(app: &App) -> Vec<String> {
    app.gui_feeds_entries()
        .iter()
        .map(|entry| entry.guid.clone())
        .collect()
}

/// The dropdown's rows as `(name, checked)` pairs, in catalog order.
fn selector_rows(projection: &GuiFeedsProjection) -> Vec<(String, bool)> {
    projection
        .collections
        .entries
        .iter()
        .map(|entry| (entry.name.clone(), entry.checked))
        .collect()
}

/// The names of the dropdown's checked subscriptions.
fn checked_names(projection: &GuiFeedsProjection) -> Vec<String> {
    projection
        .collections
        .entries
        .iter()
        .filter(|entry| entry.checked)
        .map(|entry| entry.name.clone())
        .collect()
}

/// The entry identity the frame paints (row 11.1): the subscription's URL and
/// the entry's `guid`, encoded as one opaque string.
fn entry_id(feed_id: &str, guid: &str) -> String {
    crate::app::gui_library::GuiFeedEntryId::new(feed_id.to_string(), guid.to_string()).encode()
}

/// All merges both subscriptions' entries newest first, with the undated
/// entries last (row 11.1, contract a).
#[test]
fn all_lists_both_subscriptions_newest_first_with_undated_last() {
    let app = fetched_feeds_app();

    assert_eq!(
        listed_guids(&app),
        ["a-new", "b-mid", "a-old", "a-undated", "b-undated"],
        "All merges both subscriptions newest first with nothing dated last"
    );
}

/// A checked subscription lists only that subscription's entries
/// (row 11.1, contract a).
#[test]
fn a_checked_subscription_lists_only_that_subscriptions_entries() {
    let mut app = fetched_feeds_app();

    toggle_subscription(&mut app, FEED_B);

    assert_eq!(
        listed_guids(&app),
        ["b-mid", "b-undated"],
        "the checked subscription's own list, in the same order"
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

/// A saved Feeds location applies no saved checked set (row 11.1, contract b).
#[test]
fn a_saved_feeds_location_applies_no_saved_selection() {
    let mut app = fetched_feeds_app();
    app.tab = TabSelection::Feeds;
    let saved = GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: TabIdentity::Feeds,
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
    };

    let crate::app::GuiLaunchOpen::Opened(opened) = app.open_gui_launch_destination(&saved) else {
        panic!("the feed tab is presented, so the start has opened");
    };

    assert!(
        app.restore_gui_launch_collections(&saved, &opened),
        "the Feeds destination needs no retry"
    );
    assert_eq!(
        app.gui_feeds_effective_set().as_ref(),
        &GuiCollectionSet::All,
        "the launch file saves no collection, so the restored tab opens on All"
    );
}

/// A checked subscription is stored, and one the tab no longer holds changes
/// nothing (row 11.1, contract c).
#[test]
fn a_checked_subscription_is_stored_and_a_stale_one_changes_nothing() {
    let mut app = fetched_feeds_app();

    toggle_subscription(&mut app, FEED_B);
    assert_eq!(
        app.gui_feeds_effective_set().as_ref(),
        &GuiCollectionSet::Checked(vec![FEED_B.into()]),
        "the checked subscription is stored"
    );

    toggle_subscription(&mut app, "https://example.test/gone");
    assert_eq!(
        app.gui_feeds_effective_set().as_ref(),
        &GuiCollectionSet::Checked(vec![FEED_B.into()]),
        "a subscription the tab does not hold changes nothing"
    );
}

/// A checked subscription that leaves the tab never paints (row 11.1,
/// contract c; gui-collection-dropdown design D1's vanish rule): the
/// effective read drops the check the tab no longer holds, so the last
/// checked subscription's drop returns the selection to All.
#[test]
fn a_checked_subscription_the_tab_drops_never_paints() {
    let mut app = fetched_feeds_app();
    toggle_subscription(&mut app, FEED_B);

    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A)]);

    assert_eq!(
        app.gui_feeds_effective_set().as_ref(),
        &GuiCollectionSet::All,
        "a removed subscription's check never paints"
    );
}

/// An entry pick is stored, and one the checked subscriptions' list does not
/// carry changes nothing (row 11.1, contract c).
#[test]
fn an_entry_pick_is_stored_and_one_outside_the_selected_list_changes_nothing() {
    let mut app = fetched_feeds_app();
    toggle_subscription(&mut app, FEED_B);

    app.gui_feeds_select(&entry_id(FEED_B, "b-mid"));
    assert_eq!(
        app.gui_feeds_picked_entry(),
        Some(entry_id(FEED_B, "b-mid").as_str()),
        "the picked entry is the select command's"
    );

    app.gui_feeds_select(&entry_id(FEED_A, "a-old"));
    assert_eq!(
        app.gui_feeds_picked_entry(),
        Some(entry_id(FEED_B, "b-mid").as_str()),
        "an entry the selected pill's list does not carry changes nothing"
    );
}

/// The entry row id round-trips through its one encoding (row 11.1): the
/// length prefix keeps the pair unambiguous even when the ids hold the
/// separator, and a string of another shape decodes to nothing.
#[test]
fn the_entry_row_id_round_trips_through_its_encoding() {
    let entry = crate::app::gui_library::GuiFeedEntryId::new(
        "https://example.test/a:1".into(),
        "guid:2".into(),
    );
    let encoded = entry.encode();

    assert_eq!(
        crate::app::gui_library::GuiFeedEntryId::decode(&encoded),
        Some(entry)
    );
    assert_eq!(
        crate::app::gui_library::GuiFeedEntryId::decode("no-separator"),
        None
    );
    assert_eq!(
        crate::app::gui_library::GuiFeedEntryId::decode("99:too-short"),
        None
    );
    assert_eq!(
        crate::app::gui_library::GuiFeedEntryId::decode("4:abcd"),
        None,
        "an empty guid names no entry"
    );
}

/// All merges both subscriptions' entries in the date order with the pill row
/// and the default Hero (row 11.1, contract a): newest first with the undated
/// entries last, All first in the pill row and checked, the first row as the
/// selected entry, and the Hero carrying that entry's title, its subscription
/// name, and the Square placeholder shape with no artwork.
#[test]
fn all_merges_two_subscriptions_in_date_order_with_the_default_hero() {
    let app = fetched_feeds_app();

    let projection = app.gui_feeds_projection();

    assert_eq!(
        projection
            .rows
            .iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        ["a-new", "b-mid", "a-old", "a-undated", "b-undated"],
        "All merges both subscriptions newest first with nothing dated last"
    );
    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("Feed A"),
        "the merged row names its own subscription"
    );
    assert_eq!(
        projection.rows[0].date.as_deref(),
        Some(mbv_ui_model::ui_util::fmt_publish_date_short(300).as_str()),
        "the gutter carries the publish date"
    );
    assert_eq!(
        projection.rows[4].date, None,
        "an undated entry carries no gutter"
    );
    assert_eq!(
        projection.rows[0].id,
        entry_id(FEED_A, "a-new"),
        "the row id is the composite entry identity"
    );
    assert_eq!(
        selector_rows(&projection),
        [("Feed A".to_owned(), false), ("Feed B".to_owned(), false)],
        "the dropdown offers every subscription by name, all unchecked under All"
    );
    assert!(projection.collections.all, "the selection starts on All");
    assert_eq!(
        projection.selected.as_deref(),
        Some(entry_id(FEED_A, "a-new").as_str()),
        "the first row is the Hero default"
    );

    let hero = projection.hero.expect("the Hero paints");
    assert_eq!(hero.title, "a-new");
    assert_eq!(
        hero.parent.as_deref(),
        Some("Feed A"),
        "the feed is the largest unit"
    );
    assert!(
        hero.meta_rows.is_empty(),
        "an entry of no duration adds no meta row, and the feed name is the parent"
    );
    assert_eq!(hero.duration_row, None);
    assert_eq!(hero.progress_row, None);
    assert_eq!(
        hero.art_shape,
        crate::app::GuiArtShape::Square,
        "a podcast feed reserves the Square placeholder"
    );
}

/// A checked subscription scopes the list to that subscription (row 11.1,
/// contract a): only that subscription's entries ride the rows, and its pill
/// is the one checked.
#[test]
fn a_checked_subscription_scopes_the_rows_to_that_subscription() {
    let mut app = fetched_feeds_app();

    toggle_subscription(&mut app, FEED_B);
    let projection = app.gui_feeds_projection();

    assert_eq!(
        projection
            .rows
            .iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        ["b-mid", "b-undated"],
        "a subscription pill holds that subscription's entries only"
    );
    assert_eq!(
        checked_names(&projection),
        ["Feed B"],
        "only the checked subscription's row is checked"
    );
    assert_eq!(
        projection.selected.as_deref(),
        Some(entry_id(FEED_B, "b-mid").as_str()),
        "the scoped list's first row is the Hero default"
    );
}

/// A checked subscription the tab no longer holds drops from the set
/// (row 11.1; gui-collection-dropdown design D1's vanish rule): the projection
/// paints All's merged rows and marks All checked.
#[test]
fn a_vanished_checked_subscription_falls_back_to_all() {
    let mut app = fetched_feeds_app();
    toggle_subscription(&mut app, FEED_B);
    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A)]);
    app.send_gui_feed_fetch_result_for_test(
        0,
        &[
            ("a-old", Some(100)),
            ("a-new", Some(300)),
            ("a-undated", None),
        ],
    );
    assert!(app.drain_feed_tab_results(), "the refetch drained");

    let projection = app.gui_feeds_projection();

    assert_eq!(
        projection
            .rows
            .iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        ["a-new", "a-old", "a-undated"],
        "the fallback view lists the remaining subscription"
    );
    assert_eq!(
        checked_names(&projection),
        Vec::<String>::new(),
        "All checks no subscription"
    );
}

/// Two checked subscriptions merge their entries (gui-collection-dropdown
/// design D2): both pills stay checked, and the merged list keeps the date
/// order with the undated entries last.
#[test]
fn two_checked_subscriptions_merge_their_entries_in_date_order() {
    let mut app = fetched_feeds_app();

    toggle_subscription(&mut app, FEED_A);
    toggle_subscription(&mut app, FEED_B);
    let projection = app.gui_feeds_projection();

    assert_eq!(
        projection
            .rows
            .iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        ["a-new", "b-mid", "a-old", "a-undated", "b-undated"],
        "the two checked subscriptions' entries merge in the date order"
    );
    assert_eq!(
        checked_names(&projection),
        ["Feed A", "Feed B"],
        "both subscriptions stay checked"
    );
}

/// The Oldest order puts the earliest entry first, with the undated entries
/// last (gui-collection-dropdown design D4, D5): the date-order command stores
/// the order, and the projection sorts the merged list by it without a
/// refetch.
#[test]
fn the_date_order_command_sets_the_oldest_order() {
    let mut app = fetched_feeds_app();
    app.apply_gui_library_command(GuiLibraryCommand::DateOrder {
        target: GuiCollectionTarget::Feeds,
        order: GuiDateOrder::Oldest,
    });

    assert_eq!(app.gui_feeds_date_order(), GuiDateOrder::Oldest);
    let projection = app.gui_feeds_projection();

    assert_eq!(
        projection
            .rows
            .iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        ["a-old", "b-mid", "a-new", "a-undated", "b-undated"],
        "Oldest sorts the earliest entry first and keeps nothing dated last"
    );
}

/// Two subscriptions that publish the same `guid` name different entries (row
/// 11.1): both rows ride All with different ids, and picking the second one
/// selects exactly that row, so the composite identity is what addresses one
/// entry.
#[test]
fn two_subscriptions_publishing_the_same_guid_name_different_entries() {
    let mut app = make_app_stub();
    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A), ("Feed B", FEED_B)]);
    app.tab = TabSelection::Feeds;
    app.send_gui_feed_fetch_result_for_test(0, &[("dup", Some(100))]);
    app.send_gui_feed_fetch_result_for_test(1, &[("dup", Some(200))]);
    assert!(app.drain_feed_tab_results(), "both results drained");

    let rows = app.gui_feeds_projection().rows;
    assert_eq!(rows.len(), 2, "both subscriptions' entries are rows");
    assert_eq!(rows[0].subtitle.as_deref(), Some("Feed B"));
    assert_eq!(rows[0].id, entry_id(FEED_B, "dup"));
    assert_eq!(
        rows[1].id,
        entry_id(FEED_A, "dup"),
        "the same guid in another subscription is another entry"
    );

    app.gui_feeds_select(&rows[1].id);
    let projection = app.gui_feeds_projection();
    assert_eq!(
        projection.selected.as_deref(),
        Some(rows[1].id.as_str()),
        "the pick addresses the one entry it names"
    );
    assert_eq!(projection.hero.map(|hero| hero.title), Some("dup".into()));
}

/// An entry with a resume position and no known duration is `Active` with no
/// percent, never `Ordinary` (row 7C.4): the shared
/// `MediaSemanticState::from_feed_entry` rule keeps the resume-position fact
/// separate from the percent, which only a known duration can name.
#[test]
fn a_resume_position_without_a_duration_stays_active_with_no_percent() {
    let mut app = make_app_stub();
    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A)]);
    app.tab = TabSelection::Feeds;
    let entry = mbv_queue::FeedEntry {
        guid: "half-played".to_string(),
        title: "half-played".to_string(),
        enclosure_url: Some("https://example.test/half-played.mp3".to_string()),
        link: None,
        mime_type: Some("audio/mpeg".to_string()),
        duration_ticks: None,
        pub_date_secs: Some(100),
        feed_kind: Some(mbv_queue::FeedKind::Audio),
        feed_id: None,
        position_ticks: 500,
        played: false,
    };
    app.send_gui_feed_entries_for_test(0, &[entry]);
    assert!(app.drain_feed_tab_results(), "the result drained");

    let rows = app.gui_feeds_projection().rows;

    assert_eq!(
        rows[0].state,
        MediaSemanticState::Active { progress: None },
        "the position without a duration is an Active row with no percent"
    );
}

/// One seeded entry of the subscription `feed_id`, its kind the entry's own
/// declared `FeedKind` when it carries one.
fn seeded_entry(
    guid: &str,
    pub_date_secs: u64,
    kind: Option<mbv_queue::FeedKind>,
) -> mbv_queue::FeedEntry {
    mbv_queue::FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: Some(format!("https://example.test/{guid}.mp3")),
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: Some(pub_date_secs),
        feed_kind: kind,
        feed_id: Some(FEED_A.into()),
        position_ticks: 0,
        played: false,
    }
}

/// A feed entry's row paints no art, because the feed artwork policy declares
/// no source, and shows its media kind from the entry's declared `FeedKind`
/// (gui-delineated-rows design D4, D5): a podcast feed is audio and every other
/// entry is video.
#[test]
fn a_feed_row_paints_no_art_and_its_declared_media_kind() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A)]);
    app.feed_tab.all_entries = vec![
        seeded_entry("audio-entry", 300, Some(mbv_queue::FeedKind::Audio)),
        seeded_entry("video-entry", 200, Some(mbv_queue::FeedKind::Video)),
    ];
    app.tab = TabSelection::Feeds;

    let projection = app.gui_feeds_projection();

    assert_eq!(
        projection.rows[0].art,
        GuiRowArt::NoSource,
        "the feed artwork policy declares no source"
    );
    assert_eq!(projection.rows[0].media, GuiRowMediaKind::Audio);
    assert_eq!(projection.rows[1].media, GuiRowMediaKind::Video);
    assert_eq!(
        app.images.card_image_fetch_calls(),
        0,
        "a row with no source reserves no fetch"
    );
}
