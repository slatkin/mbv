//! The item menus the frame carries (add-gui-library-panel design D8, D2):
//! the Hero's entries, the right-click row menu, and the entries a stale id
//! carries none of.

use mbv::ClientTickHost;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_net::mock_http::MockHttp;
use mbv_queue::QueueSource;

use super::{
    TRACKS, music_detail, replacement, seeded_music_core, send_library, send_library_frame,
};

/// The item menu (design D8): the frame carries the picked album's entries,
/// and a chosen entry runs through the shared context-action executor for that
/// album. The GUI never arms the TUI's `pending_overlay`.
#[test]
fn the_item_menu_entries_ride_the_frame_and_run_their_action() {
    let http = MockHttp::new();
    http.respond(200, TRACKS);
    let (mut test, cmd_rx) = seeded_music_core(&http);
    send_library_frame(&mut test, GuiLibraryCommand::TakeFocus);
    send_library_frame(
        &mut test,
        GuiLibraryCommand::SelectCollection {
            lib_idx: 0,
            group_id: "grp-hiphop".into(),
        },
    );
    let picked = send_library_frame(
        &mut test,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-paul".into(),
        },
    );

    let menu = music_detail(&picked)
        .and_then(|detail| detail.hero)
        .expect("the picked album has a Hero")
        .menu;
    let labels: Vec<&str> = menu.iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(
        labels,
        ["Play All", "Shuffle", "Add to Queue"],
        "the frame carries the album's own entries"
    );

    send_library(
        &mut test,
        GuiLibraryCommand::MenuEntry {
            item_id: "alb-ill".into(),
            index: 0,
        },
    );

    let (source, ids) = replacement(&cmd_rx);
    assert_eq!(
        ids,
        ["trk-1", "trk-2"],
        "the Play All entry played the picked album"
    );
    assert_eq!(
        source,
        QueueSource::Collection {
            collection_type: "music".into()
        }
    );
    assert!(
        test.core.app_mut().pending_overlay_is_none_for_test(),
        "the GUI runs the entry without arming the TUI overlay"
    );
}

/// The right-click item menu (the row-menu command): the frame carries the
/// named item's full TUI-menu entries — including the entries the Hero's
/// buttons send — against that item's stable id.
#[test]
fn the_row_menu_command_carries_the_items_full_menu_in_the_frame() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = seeded_music_core(&http);
    send_library_frame(&mut test, GuiLibraryCommand::TakeFocus);
    let frame = send_library_frame(
        &mut test,
        GuiLibraryCommand::RowMenu {
            item_id: "alb-ill".into(),
        },
    );

    let row_menu = frame
        .row_menu()
        .cloned()
        .expect("the right-clicked item has a menu");
    assert_eq!(row_menu.item_id, "alb-ill");
    let labels: Vec<&str> = row_menu
        .entries
        .iter()
        .map(|entry| entry.label.as_str())
        .collect();
    assert_eq!(labels, ["Play All", "Shuffle", "Add to Queue"],);
}

/// Entities a right-click menu names but the store no longer holds carry no
/// menu (design D2: a stale frame addresses the item it painted or nothing);
/// and there is no Music added toggling.
#[test]
fn a_row_menu_command_for_an_abandoned_item_carries_no_menu() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = seeded_music_core(&http);
    send_library_frame(&mut test, GuiLibraryCommand::TakeFocus);
    let frame = send_library_frame(
        &mut test,
        GuiLibraryCommand::RowMenu {
            item_id: "alb-gone".into(),
        },
    );

    assert!(
        frame.row_menu().is_none(),
        "a stale right-click carries no menu"
    );
}
