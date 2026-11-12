//! The Feeds destination's contracts (add-gui-library-panel design D12, row
//! 11.1): one background fetch's completion lands on the App through the GUI
//! tick's own drain, so the destination's list reads the same App data the TUI
//! shell reads; the frame carries the pill row and the active pill's entry
//! rows once the destination opens; a pill command re-scopes the rows; a
//! select moves the Hero; and the play and queue commands reach the Owner
//! through the TUI's feed paths. The fetch boundary is the App's own result
//! channel, so no request leaves the process and no worker outlives the
//! assertion.

use mbv::ClientTickHost;
use mbv_ctrl::CtrlCmd;
use mbv_gui::bridge::frame::library::GuiFeedsFrame;
use mbv_gui::bridge::library::GuiFeedPill;
use mbv_queue::FeedEntry;
use mbv_ui_model::tab_selection::TabSelection;

use super::{library_of, send_library};

/// The two subscriptions the tests configure: the feed identity is the URL, as
/// the fetch stamps it on every entry.
const FEED_A: &str = "https://example.test/a";
const FEED_B: &str = "https://example.test/b";

/// A feed fetch's completion lands on the App through the GUI tick's own
/// drain (row 11.1): the destination's list reads the same App data the TUI
/// shell reads.
#[test]
fn a_feed_fetch_result_lands_on_the_app_through_the_gui_tick() {
    let mut test = crate::test_core(0);
    test.core.app_mut().seed_gui_feeds_for_test(&[
        ("Feed A", "https://example.test/a"),
        ("Feed B", "https://example.test/b"),
    ]);
    test.core
        .app_mut()
        .send_gui_feed_fetch_result_for_test(0, &[("a-old", Some(100)), ("a-new", Some(300))]);
    test.core
        .app_mut()
        .send_gui_feed_fetch_result_for_test(1, &[("b-mid", Some(200)), ("b-undated", None)]);

    assert!(test.core.step(), "the core thread never quits mid-drain");

    let app = test.core.app_mut();
    let guids: Vec<String> = app
        .gui_feeds_entries()
        .iter()
        .map(|entry| entry.guid.clone())
        .collect();
    assert_eq!(
        guids,
        ["a-new", "b-mid", "a-old", "b-undated"],
        "the tick's drain landed both fetches, merged newest first with the undated entry last"
    );
}

/// A core over an App with no Service library and two configured
/// subscriptions, over the recording stub Owner the queue commands' tests read.
fn seeded_feeds_core() -> (crate::TestCore, std::sync::mpsc::Receiver<CtrlCmd>) {
    let (app, cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = crate::test_core_with(app);
    test.core
        .app_mut()
        .seed_gui_feeds_for_test(&[("Feed A", FEED_A), ("Feed B", FEED_B)]);
    (test, cmd_rx)
}

/// Land both subscriptions' fetch results on the App through the tick's drain:
/// the CSV of the mocked background fetches, with no worker behind them.
fn land_fetches(test: &mut crate::TestCore) {
    let app = test.core.app_mut();
    app.send_gui_feed_fetch_result_for_test(
        0,
        &[
            ("a-old", Some(100)),
            ("a-new", Some(300)),
            ("a-undated", None),
        ],
    );
    app.send_gui_feed_fetch_result_for_test(1, &[("b-mid", Some(200)), ("b-undated", None)]);
    assert!(test.core.step(), "the core thread never quits mid-drain");
}

/// Open the Feeds destination and return the tab position the open command
/// named (design D2, D12).
fn open_feeds(test: &mut crate::TestCore) -> usize {
    let position = test
        .core
        .app_mut()
        .gui_library_tabs()
        .tabs
        .iter()
        .find(|tab| tab.label == "Feeds")
        .map(|tab| tab.position)
        .expect("the presented tabs carry the Feeds tab");
    send_library(
        test,
        mbv_gui::bridge::library::GuiLibraryCommand::OpenDestination { position },
    );
    position
}

/// The newest frame the core emitted, draining the older ones: the frames the
/// core emitted before the seed stay queued, so a test reads the frame its last
/// step built. The read takes no step of its own.
fn newest_frame(test: &mut crate::TestCore) -> crate::Held {
    test.held()
}

/// Report a layout, as a window content reload does, so the Library section
/// re-carries the content it built from the seeded data: the reload re-arms the
/// memo, and the report's own step emits the frame that carries it.
fn reloaded_frame(test: &mut crate::TestCore) -> crate::Held {
    test.tx
        .send(crate::measured_layout(10, Vec::new()))
        .unwrap();
    assert!(test.core.step());
    newest_frame(test)
}

/// The Feeds section a frame carries (row 11.1).
fn feeds_of(frame: &crate::Held) -> GuiFeedsFrame {
    library_of(frame)
        .and_then(|content| content.feeds)
        .expect("the frame carries the Feeds section")
}

/// The entry ids a queue command named, from the recording stub Owner's
/// channel: one feed entry's `guid` per item.
fn commanded_feed_guids(cmd_rx: &std::sync::mpsc::Receiver<CtrlCmd>) -> Vec<String> {
    cmd_rx
        .try_iter()
        .find_map(|command| match command {
            CtrlCmd::UnifiedQueueReplace { items, .. }
            | CtrlCmd::UnifiedQueueAppend { items, .. } => Some(
                items
                    .iter()
                    .filter_map(|item| match item {
                        mbv_queue::QueueItem::Feed(entry) => Some(entry.guid.clone()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .expect("the Owner received the entry's queue command")
}

/// The frame carries the Feeds list after the open (row 11.1): the entries are
/// in the TUI's date order with the undated ones last, the pill row is Latest
/// first then each subscription by name, the first row is the Hero default,
/// and the Hero carries that entry's title, its subscription, and the
/// placeholder shape.
#[test]
fn the_frame_carries_the_feed_list_after_the_open() {
    let (mut test, _cmd_rx) = seeded_feeds_core();
    open_feeds(&mut test);
    land_fetches(&mut test);

    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::Feeds,
        "the Feeds destination is active"
    );
    let feeds = feeds_of(&reloaded_frame(&mut test));
    let titles: Vec<&str> = feeds.rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(
        titles,
        ["a-new", "b-mid", "a-old", "a-undated", "b-undated"],
        "Latest merges both subscriptions newest first with nothing dated last"
    );
    assert_eq!(
        feeds
            .pills
            .iter()
            .map(|pill| (pill.name.as_str(), pill.selected))
            .collect::<Vec<_>>(),
        [("Latest", true), ("Feed A", false), ("Feed B", false)],
        "the pill row is Latest first, then each subscription by name"
    );
    assert_eq!(
        feeds.selected.as_deref(),
        feeds.rows.first().map(|row| row.id.as_str()),
        "the first row is the Hero default"
    );
    let hero = feeds.hero.expect("the Hero paints");
    assert_eq!(hero.title, "a-new");
    assert_eq!(hero.parent.as_deref(), Some("Feed A"));
    assert_eq!(
        hero.art_shape,
        mbv_gui::bridge::frame::library::GuiArtShapeFrame::Square
    );
}

/// A pill command re-scopes the rows (row 11.1): the set-pill command's
/// subscription leaves only that subscription's entries, and its pill is the
/// selected one.
#[test]
fn the_pill_command_changes_the_rows() {
    let (mut test, _cmd_rx) = seeded_feeds_core();
    open_feeds(&mut test);
    land_fetches(&mut test);

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::FeedsSetPill {
            pill: GuiFeedPill::Subscription(FEED_B.into()),
        },
    );

    let feeds = feeds_of(&newest_frame(&mut test));
    let titles: Vec<&str> = feeds.rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles, ["b-mid", "b-undated"]);
    let selected: Vec<&str> = feeds
        .pills
        .iter()
        .filter(|pill| pill.selected)
        .map(|pill| pill.name.as_str())
        .collect();
    assert_eq!(selected, ["Feed B"]);
}

/// A select command moves the Hero (row 11.1): the second row's opaque id
/// becomes the frame's selection, and the Hero shows that entry.
#[test]
fn the_select_command_moves_the_hero() {
    let (mut test, _cmd_rx) = seeded_feeds_core();
    open_feeds(&mut test);
    land_fetches(&mut test);
    let row = feeds_of(&reloaded_frame(&mut test))
        .rows
        .get(1)
        .map(|row| row.id.clone())
        .expect("the list holds the second row");

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::FeedsSelect { row: row.clone() },
    );

    let feeds = feeds_of(&newest_frame(&mut test));
    assert_eq!(feeds.selected.as_deref(), Some(row.as_str()));
    let hero = feeds.hero.expect("the Hero paints");
    assert_eq!(hero.title, "b-mid");
    assert_eq!(hero.parent.as_deref(), Some("Feed B"));
}

/// The play and queue commands reach the Owner through the TUI's feed paths
/// (row 11.1): the play replaces the Owner's queue with the entry and the
/// queue command appends it.
#[test]
fn the_play_and_queue_commands_reach_the_owner() {
    let (mut test, cmd_rx) = seeded_feeds_core();
    open_feeds(&mut test);
    land_fetches(&mut test);
    let row = feeds_of(&reloaded_frame(&mut test))
        .rows
        .first()
        .map(|row| row.id.clone())
        .expect("the list holds rows");

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::FeedsPlay {
            item_id: row.clone(),
        },
    );
    assert_eq!(
        commanded_feed_guids(&cmd_rx),
        ["a-new"],
        "the play replaced the Owner's queue with the entry"
    );

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::FeedsQueue { item_id: row },
    );
    assert_eq!(
        commanded_feed_guids(&cmd_rx),
        ["a-new"],
        "the queue command appended the entry"
    );
}

/// An entry with no playable source is refused as the TUI refuses it (row
/// 11.1): the play flashes the TUI's toast and submits nothing.
#[test]
fn a_source_less_entry_flashes_and_submits_nothing() {
    let (mut test, cmd_rx) = seeded_feeds_core();
    open_feeds(&mut test);
    test.core.app_mut().send_gui_feed_entries_for_test(
        1,
        &[FeedEntry {
            guid: "b-sourceless".into(),
            title: "b-sourceless".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: Some(400),
            feed_kind: Some(mbv_queue::FeedKind::Audio),
            feed_id: None,
            position_ticks: 0,
            played: false,
        }],
    );
    assert!(test.core.step(), "the core thread never quits mid-drain");
    let row = feeds_of(&reloaded_frame(&mut test))
        .rows
        .first()
        .map(|row| row.id.clone())
        .expect("the list holds the source-less row");

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::FeedsPlay { item_id: row },
    );

    assert!(cmd_rx.try_recv().is_err(), "the refusal submits nothing");
    assert!(
        newest_frame(&mut test).status().toast.is_some(),
        "it flashes"
    );
}

/// Stale Feeds row ids change nothing (row 11.1): a play of a row id of no
/// valid shape submits nothing and flashes no toast, and so does a play of a
/// well-formed id no list holds.
#[test]
fn a_stale_feed_row_submits_nothing() {
    let (mut test, cmd_rx) = seeded_feeds_core();
    open_feeds(&mut test);
    land_fetches(&mut test);
    let _ = reloaded_frame(&mut test);

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::FeedsPlay {
            item_id: "not-a-row-id".into(),
        },
    );
    assert!(cmd_rx.try_recv().is_err(), "a malformed id submits nothing");
    assert!(newest_frame(&mut test).status().toast.is_none());

    let absent = mbv::GuiFeedEntryId::new(FEED_A.into(), "gone".into()).encode();
    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::FeedsQueue { item_id: absent },
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "an id no list holds submits nothing"
    );
    assert!(newest_frame(&mut test).status().toast.is_none());
}
