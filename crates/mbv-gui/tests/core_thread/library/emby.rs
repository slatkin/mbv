//! The GUI Library's plain Emby-library contracts through the GUI tick
//! (add-gui-library-panel design D12, row 8.1): the mocked bootstrap's open
//! Movies destination finds the `emby` frame, a select command moves the
//! Hero, and an `embySetFeedGroup` pill switch loads the chosen group's items
//! into the next frame. The Emby answers come from the in-memory mock
//! transport, so no request leaves the process, and no live Owner, Service,
//! or webview runs (design D13).

use crate::Held;
use mbv::ClientTickHost;
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_gui::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::feed::FeedHomeVideoSelection;

use super::{TestCore, pump_until, send_library};
use crate::{test_core_with, test_emby_client};

/// The library's one listing, as the mock transport returns it: the body the
/// root listing, the folder-count query, and the selected group's items
/// fetch all parse — the listing's folder is the one counted folder, and the
/// video is the item the switched group's rows carry.
const LIBRARY_BODY: &str = r#"{"Items":[{"Id":"folder-1","Name":"Channel 1","Type":"Folder"},{"Id":"vid-1","Name":"Clip","Type":"Video"}],"TotalRecordCount":2}"#;

/// A Latest snapshot listing, as the mock transport returns it: the bare
/// JSON array `get_latest` parses, whose video sits in the listing's folder
/// and carries the date added the New rows' gutter reads.
const LATEST_BODY: &str = r#"[{"Id":"vid-new","Name":"Fresh Clip","Type":"Video","ParentId":"folder-1","DateCreated":"2026-11-01T10:00:00.0000000Z"}]"#;

/// A movie row of a root-list store's loaded list.
fn movie(id: &str, name: &str) -> EmbyItem {
    let mut item = mbv_emby_model::test_support::make_item(name, "Movie");
    item.id = id.to_string();
    item
}

/// The plain Emby-library section a frame carries, present only while a
/// plain Emby-library destination presents.
fn emby_section(frame: &Held) -> Option<&mbv_gui::bridge::frame::library::GuiEmbyFrame> {
    frame.library()?.emby.as_ref()
}

/// Pump GUI ticks until the plain Emby-library section a frame carries
/// satisfies `want`, waiting for the next library completion between steps
/// (the fetch workers' own progress) instead of sleeping between them.
fn pump_until_emby(
    test: &mut TestCore,
    want: impl Fn(&mbv_gui::bridge::frame::library::GuiEmbyFrame) -> bool,
    what: &str,
) {
    pump_until(test, |frame| emby_section(frame).is_some_and(&want), what);
}

/// The mocked bootstrap's catalog presents the Movies destination, and the
/// restore's open finds the `emby` frame (row 8.1, design D12): the plain
/// mode's section names the library and carries no pill, and the root-list
/// fetch fails against the no-URL stub client; a select command over the
/// seeded store then moves the Hero to that row.
#[test]
fn the_bootstrap_movies_destination_finds_the_emby_frame_and_a_select_moves_the_hero() {
    let view = EmbyItem::folder(
        "lib-movies".to_string(),
        "Movies".to_string(),
        "movies".to_string(),
    );
    let mut test = test_core_with(mbv::stub_app_with_completed_emby_startup(vec![view]));

    // The mocked bootstrap drains on the first tick; the catalog's one
    // presented destination is the Movies library, and the launch restore's
    // first-presented open activates it.
    assert!(test.core.step());
    let frame = test.held();
    let emby = emby_section(&frame).expect("the open Movies destination presents its content");
    assert_eq!(
        emby.lib_idx, 0,
        "the frame names the library the commands carry"
    );
    assert!(!emby.feed_group, "the Movies library is the plain mode");
    assert!(
        emby.pills.is_empty() && emby.rows.is_empty() && emby.hero.is_none(),
        "the root list has not landed rows yet"
    );
    // The no-URL stub client's root-list fetch fails; the frame names the
    // failure once it lands, and a later frame build re-issues nothing
    // behind the seed below.
    pump_until_emby(&mut test, |emby| emby.failed, "the failed root fetch");

    test.core
        .app_mut()
        .seed_gui_emby_roots_for_test(0, vec![movie("m-1", "A Movie"), movie("m-2", "B Movie")]);
    send_library(
        &mut test,
        GuiLibraryCommand::EmbySelect {
            lib_idx: 0,
            row: "m-2".into(),
        },
    );
    let frame = test.held();
    let emby = emby_section(&frame).expect("the frame carries the Emby content");
    assert_eq!(
        emby.selected.as_deref(),
        Some("m-2"),
        "the frame paints the core's selection"
    );
    assert_eq!(
        emby.hero.as_ref().expect("the Hero moved").title,
        "B Movie",
        "the Hero's facts follow the selected row"
    );
}

/// An `embySetFeedGroup` pill switch loads the chosen group's items into the
/// next frame (row 8.1, the task's own contract): the set-feed-group command
/// runs the TUI's own selection path, whose lazy fetch the mock answers, and
/// the frame path's projection carries the folder pill as the selected one
/// with the group's items as the rows.
#[test]
fn an_emby_set_feed_group_command_loads_the_groups_items_into_the_next_frame() {
    let http = MockHttp::new();
    http.respond_repeated(200, LIBRARY_BODY, 12);
    let (app, _cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    // The All selection arrives cached (empty), so the folder fetch the
    // switched group starts is the one items fetch the mock answers.
    test.core
        .app_mut()
        .seed_gui_feed_view_library_for_test(test_emby_client(&http));

    // The restore's open re-activates the position, whose root listing and
    // count fan-out land through the tick's drains.
    pump_until_emby(
        &mut test,
        |emby| emby.pills.len() == 3 && !emby.loading,
        "the counted folder pill",
    );

    send_library(
        &mut test,
        GuiLibraryCommand::EmbySetFeedGroup {
            lib_idx: 0,
            group: mbv_gui::bridge::library::GuiEmbyFeedGroup::Folder("folder-1".into()),
        },
    );
    pump_until_emby(
        &mut test,
        |emby| {
            emby.rows.iter().any(|row| row.id == "vid-1")
                && emby
                    .pills
                    .iter()
                    .find(|pill| pill.selected)
                    .is_some_and(|pill| pill.name == "Channel 1")
        },
        "the switched group's items",
    );
    let app = test.core.app_mut();
    assert!(
        app.libs_for_test()[0].feed_home_video.as_ref().is_some_and(
            |state| state.selection == FeedHomeVideoSelection::Folder("folder-1".into())
        ),
        "the feed state's selection is the command's folder"
    );
}

/// An `embySetFeedGroup` New-pill press selects the Latest view and loads the
/// snapshot into the next frame (row 8.1, the New pill's contract): the
/// command triggers the store's own Latest fetch, whose items are the frame's
/// rows with the New pill the selected one, and the rows carry the programme
/// (root folder) the video sits in underneath the title.
#[test]
fn an_emby_new_pill_command_loads_the_latest_snapshot_into_the_next_frame() {
    let http = MockHttp::new();
    // The Latest fetch answers a bare JSON array, the body `get_latest`
    // parses; every other request this open path makes reads the listing.
    http.respond_when("/Items/Latest", 200, LATEST_BODY);
    http.respond_repeated(200, LIBRARY_BODY, 12);
    let (app, _cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    test.core
        .app_mut()
        .seed_gui_feed_view_library_for_test(test_emby_client(&http));

    pump_until_emby(
        &mut test,
        |emby| emby.pills.len() == 3 && !emby.loading,
        "the counted folder pill",
    );

    send_library(
        &mut test,
        GuiLibraryCommand::EmbySetFeedGroup {
            lib_idx: 0,
            group: mbv_gui::bridge::library::GuiEmbyFeedGroup::Latest,
        },
    );
    let new_view = std::cell::RefCell::new(None);
    pump_until_emby(
        &mut test,
        |emby| {
            let ready = emby
                .pills
                .iter()
                .find(|pill| pill.selected)
                .is_some_and(|pill| pill.name == "New")
                && emby.rows.iter().any(|row| row.id == "vid-new");
            if ready {
                *new_view.borrow_mut() = Some(emby.clone());
            }
            ready
        },
        "the Latest snapshot's rows",
    );
    let emby = new_view
        .into_inner()
        .expect("the New view's frame carried its content");
    assert_eq!(
        emby.collection_names,
        vec!["New".to_string()],
        "the location names the New pill"
    );
    assert_eq!(
        emby.rows[0].subtitle,
        Some("Channel 1".to_string()),
        "the row names the programme (root folder) the video sits in"
    );
}

/// Opens the mocked Movies destination with `items` as its root list and
/// selects the first one, so the next frame carries that item's Hero.
fn movies_hero_frame(items: Vec<EmbyItem>) -> Held {
    let view = EmbyItem::folder(
        "lib-movies".to_string(),
        "Movies".to_string(),
        "movies".to_string(),
    );
    let mut test = test_core_with(mbv::stub_app_with_completed_emby_startup(vec![view]));
    assert!(test.core.step());
    pump_until_emby(&mut test, |emby| emby.failed, "the failed root fetch");
    test.core.app_mut().seed_gui_emby_roots_for_test(0, items);
    send_library(
        &mut test,
        GuiLibraryCommand::EmbySelect {
            lib_idx: 0,
            row: "m-1".into(),
        },
    );
    test.held()
}

/// An Emby Hero's duration row reaches the frame as a `time` segment, so the
/// window can paint it in the time colour (design D14).
#[test]
fn an_emby_heros_duration_row_arrives_as_a_time_segment() {
    let mut item = movie("m-1", "A Movie");
    item.runtime_ticks = 5_400 * TICKS_PER_SECOND;
    let frame = movies_hero_frame(vec![item]);
    let hero = emby_section(&frame)
        .and_then(|emby| emby.hero.as_ref())
        .expect("the Hero paints");
    let duration = hero.duration_row.expect("the Hero names its duration row");
    assert_eq!(
        hero.meta_rows[duration],
        vec![GuiMetaSegmentFrame {
            text: "01:30:00".into(),
            kind: GuiMetaKindFrame::Time,
        }],
    );
}
