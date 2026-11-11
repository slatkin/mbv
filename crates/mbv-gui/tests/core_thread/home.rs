//! Home's stored content (add-gui-library-panel row 6.1): the mocked
//! asynchronous Emby bootstrap completes on the core thread's own tick, and
//! the frame that tick emits reads the Continue Watching items the App
//! stores. The completion rides the queued startup channel, so no live
//! Owner, Service, or webview runs; the state directory is a
//! `TestStateDirGuard` scratch path (design D13).

use mbv::{ClientTickHost, stub_app_with_completed_emby_startup_and_contents};
use mbv_emby_model::test_support::make_item;
use mbv_gui::bridge::frame::GuiFrame;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::core_thread::CoreInput;

use crate::test_core_with;

/// The contents the Home frame tests seed: two Continue Watching movies.
fn home_contents() -> Vec<mbv_emby_model::EmbyItem> {
    let mut first = make_item("Continue First", "Movie");
    first.id = "cw-first".to_string();
    let mut second = make_item("Continue Second", "Movie");
    second.id = "cw-second".to_string();
    vec![first, second]
}

/// Open Home (position 0, design D2) over the core whose App the bootstrap
/// completes on the same first tick (design D13): the command rides the
/// queued channel ahead of the step, so the frame that tick emits presents
/// the Home section.
fn home_core_with_open() -> crate::TestCore {
    let mut test = test_core_with(stub_app_with_completed_emby_startup_and_contents(
        Vec::new(),
        home_contents(),
    ));
    test.tx
        .send(CoreInput::Library(
            GuiLibraryCommand::OpenDestination { position: 0 }.into(),
        ))
        .unwrap();
    assert!(test.core.step());
    test
}

/// The Home section a frame carries, present only while Home presents.
fn home_section(frame: GuiFrame) -> mbv_gui::bridge::frame::library::GuiHomeFrame {
    frame
        .library
        .expect("the slice carries the Library section")
        .content
        .expect("the open's frame carries the changed content")
        .home
        .expect("the open's frame carries the Home content")
}

/// The contents a queued refresh carries: different Continue Watching movies
/// than the startup snapshot, so the test tells the refreshed rows apart.
fn refreshed_contents() -> Vec<mbv_emby_model::EmbyItem> {
    let mut third = make_item("Continue Third", "Movie");
    third.id = "cw-third".to_string();
    let mut fourth = make_item("Continue Fourth", "Movie");
    fourth.id = "cw-fourth".to_string();
    vec![third, fourth]
}

/// An asynchronous Emby bootstrap completes on the startup drain, and the
/// next frame's tick reads its Continue Watching items from the App's stored
/// Home content (row 6.1): the later Home projection of row 6.2 reads the
/// stored snapshot, never a per-Client copy.
#[test]
fn an_emby_startup_completion_stores_the_continue_watching_for_the_next_frame() {
    let mut first = make_item("Continue First", "Movie");
    first.id = "cw-first".to_string();
    let mut second = make_item("Continue Second", "Movie");
    second.id = "cw-second".to_string();
    let contents = vec![first, second];
    let mut test = test_core_with(stub_app_with_completed_emby_startup_and_contents(
        Vec::new(),
        contents,
    ));

    // The zeroed-cadence step drains the queued completion and emits the
    // frame that follows it.
    assert!(test.core.step());
    let _frame = test.next_frame();

    let home = test.core.app_mut().home_content_for_test();
    let names: Vec<&str> = home
        .continue_items
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, vec!["Continue First", "Continue Second"]);
    assert!(
        !home.loading,
        "the stored snapshot's loading flag settles with the bootstrap"
    );
}

/// A mocked asynchronous bootstrap's `Recently added` fetch lands on the lib
/// channel, and the next tick's drain accepts it into the App's stored rows
/// exactly as the TUI shell's drain does (add-gui-library-panel row 6.2):
/// the later Home projection reads the App's store, not a per-Client copy.
/// The result rides the channel itself — no worker thread, no Service — and
/// the Audiobookshelf rows carry their provider timestamps in seconds, so
/// the 14-day cutoff needs no date-string parse.
#[test]
fn a_recently_added_fetch_result_lands_in_the_app_on_the_next_tick() {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let recent_episode = mbv_queue::QueueItem::Audiobookshelf(
        mbv_queue::AudiobookshelfItem::Episode(mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "pod-lib".into(),
            episode_id: "recent-episode".into(),
            title: "Episode".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: Some(now_secs - 60),
            is_finished: false,
            cover_path: None,
        }),
    );
    let mut app = stub_app_with_completed_emby_startup_and_contents(Vec::new(), Vec::new());
    app.queue_home_recently_added_for_test(vec![recent_episode]);
    let mut test = test_core_with(app);

    // The step drains the queued completion and then the queued fetch
    // result, so one tick accepts the rows.
    assert!(test.core.step());
    let _frame = test.next_frame();

    let ids = test.core.app_mut().home_recently_added_ids_for_test();
    assert_eq!(ids, vec!["recent-episode"]);
    assert!(
        !test.core.app_mut().home_content_for_test().loading,
        "the bootstrap the same drain ran settled Home's loading flag"
    );
}

/// A Home content refresh event on the lib channel lands in the App's
/// stored Home content on the next tick (add-gui-library-panel row 6.2),
/// so the next Home frame's `Continue watching` rows read the refreshed
/// snapshot instead of the startup one. Regression for the GUI's frozen
/// Continue Watching report of 2026-11-09: the App arms used to defer both
/// events to the shell drain a GUI core thread never runs.
#[test]
fn a_home_content_refresh_event_lands_in_the_next_home_frame() {
    let mut app = stub_app_with_completed_emby_startup_and_contents(Vec::new(), home_contents());
    app.queue_home_content_refreshed_for_test(refreshed_contents());
    let mut test = test_core_with(app);
    test.tx
        .send(CoreInput::Library(
            GuiLibraryCommand::OpenDestination { position: 0 }.into(),
        ))
        .unwrap();

    // The zeroed-cadence step drains the queued refresh, opens Home, and
    // emits the frame that follows both.
    assert!(test.core.step());
    let home = home_section(test.next_frame());

    let titles: Vec<&str> = home
        .rows
        .iter()
        .filter_map(|row| match row {
            mbv_gui::bridge::frame::library::GuiHomeRowFrame::Item(item) => {
                Some(item.title.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        titles,
        vec!["Continue Third", "Continue Fourth"],
        "the frame paints the refreshed snapshot, not the startup one"
    );
    assert_eq!(
        home.hero.expect("the Hero paints").title,
        "Continue Third",
        "the Hero reads the refreshed snapshot's first row"
    );
}

/// A Home content clear event on the lib channel empties the App's stored
/// Home content on the next tick (add-gui-library-panel row 6.2): the
/// Service teardown's GUI arm runs the same App write the shell's drain
/// does, so the next Home frame carries no rows.
#[test]
fn a_home_content_cleared_event_empties_the_next_home_frame() {
    let mut test = home_core_with_open();
    assert!(
        !home_section(test.next_frame()).rows.is_empty(),
        "the seeded snapshot painted rows before the clear"
    );

    test.core.app_mut().queue_home_content_cleared_for_test();
    assert!(test.core.step());
    let home = home_section(test.next_frame());

    assert!(
        home.rows.is_empty(),
        "the clear emptied the Continue watching section"
    );
    assert!(home.hero.is_none(), "no row survives to select");
}

/// An open-destination command for Home (position 0) presents the Home
/// frame (row 6.2, design D12): the flat list's `Continue watching` heading
/// row and item rows carry the stored snapshot, with the first item row
/// selected — Home's Hero default, so a window that opens Home has a Hero.
#[test]
fn an_open_home_destination_finds_the_home_frame() {
    let mut test = home_core_with_open();
    assert_eq!(
        test.core.app_mut().tab_for_test(),
        mbv_ui_model::tab_selection::TabSelection::Home,
        "the position-0 open made Home the active destination"
    );

    let home = home_section(test.next_frame());

    assert_eq!(
        home.rows.len(),
        3,
        "the heading row, then the two item rows"
    );
    assert!(
        matches!(
            &home.rows[0],
            mbv_gui::bridge::frame::library::GuiHomeRowFrame::Heading { .. }
        ),
        "the section opens with its heading row"
    );
    assert_eq!(
        home.selected,
        Some(mbv_gui::bridge::frame::library::GuiHomeRowIdFrame::Continue("cw-first".into())),
        "the first item row is the Hero default"
    );
    assert_eq!(
        home.hero.expect("the Hero paints").title,
        "Continue First",
        "the Hero's facts read the selected row's item"
    );
}

/// A select command moves the Hero (row 6.2, design D2, D12): the command
/// names the row the frame painted, by stable id, and the next frame's
/// `selected` and Hero facts carry that row.
#[test]
fn a_home_select_command_moves_the_hero() {
    let mut test = home_core_with_open();
    let home = home_section(test.next_frame());
    let second = match &home.rows[2] {
        mbv_gui::bridge::frame::library::GuiHomeRowFrame::Item(item) => item.id.clone(),
        _ => panic!("the second item row carries an identity"),
    };

    test.tx
        .send(CoreInput::Library(
            GuiLibraryCommand::HomeSelect {
                row: mbv_gui::bridge::library::GuiHomeRowId::Continue("cw-second".into()),
            }
            .into(),
        ))
        .unwrap();
    assert!(test.core.step());
    let home = home_section(test.next_frame());

    assert_eq!(
        home.selected,
        Some(mbv_gui::bridge::frame::library::GuiHomeRowIdFrame::Continue("cw-second".into())),
        "the frame paints the core's selection"
    );
    assert_eq!(
        home.hero.expect("the Hero paints").title,
        "Continue Second",
        "the Hero's facts follow the selected row"
    );
    // `second`'s frame id round-trips the same identity the command carries.
    assert_eq!(
        second,
        mbv_gui::bridge::frame::library::GuiHomeRowIdFrame::Continue("cw-second".into())
    );
}
