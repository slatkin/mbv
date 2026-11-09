//! The GUI Library's plain Emby-library contracts through the GUI tick
//! (add-gui-library-panel design D12, row 8.1): the mocked bootstrap's open
//! Movies destination finds the `emby` frame, a select command moves the
//! Hero, and an `embySetFeedGroup` pill switch loads the chosen group's items
//! into the next frame. The Emby answers come from the in-memory mock
//! transport, so no request leaves the process, and no live Owner, Service,
//! or webview runs (design D13).

use mbv::ClientTickHost;
use mbv_emby_model::EmbyItem;
use mbv_gui::bridge::frame::GuiFrame;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::feed::FeedHomeVideoSelection;

use super::{TestCore, send_library};
use crate::{test_core_with, test_emby_client};

/// The library's one listing, as the mock transport returns it: the body the
/// root listing, the folder-count query, and the selected group's items
/// fetch all parse — the listing's folder is the one counted folder, and the
/// video is the item the switched group's rows carry.
const LIBRARY_BODY: &str = r#"{"Items":[{"Id":"folder-1","Name":"Channel 1","Type":"Folder"},{"Id":"vid-1","Name":"Clip","Type":"Video"}],"TotalRecordCount":2}"#;

/// A movie row of a root-list store's loaded list.
fn movie(id: &str, name: &str) -> EmbyItem {
    let mut item = mbv_emby_model::test_support::make_item(name, "Movie");
    item.id = id.to_string();
    item
}

/// The plain Emby-library section a frame carries, present only while a
/// plain Emby-library destination presents.
fn emby_section(frame: &GuiFrame) -> Option<&mbv_gui::bridge::frame::library::GuiEmbyFrame> {
    frame.library.as_ref()?.content.as_ref()?.emby.as_ref()
}

/// Pump GUI ticks until the plain Emby-library section a frame carries
/// satisfies `want`, bounded so a broken fetch fails the assertion instead
/// of hanging the test. Each miss waits briefly on the mock for one more
/// transmitted request — the sanctioned seam for the fetch workers' own
/// progress, as the Shows tests run it.
fn pump_until_emby(
    test: &mut TestCore,
    http: &MockHttp,
    want: impl Fn(&mbv_gui::bridge::frame::library::GuiEmbyFrame) -> bool,
    what: &str,
) {
    for _ in 0..250 {
        assert!(test.core.step(), "the core thread never quits mid-drain");
        if let Some(emby) = test.latest_frame().as_ref().and_then(emby_section)
            && want(emby)
        {
            return;
        }
        let _ = http.wait_for_requests(
            http.request_count() + 1,
            std::time::Duration::from_millis(50),
        );
    }
    panic!("the tick never produced {what}");
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
    let frame = test.latest_frame().expect("the bootstrap's frame emitted");
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
    pump_until_emby(
        &mut test,
        &MockHttp::new(),
        |emby| emby.failed,
        "the failed root fetch",
    );

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
    let frame = test.latest_frame().expect("the select's frame emitted");
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
        &http,
        |emby| emby.pills.len() == 2 && !emby.loading,
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
        &http,
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
