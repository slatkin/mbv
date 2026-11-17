//! The destination's stored selection and detail (add-gui-library-panel
//! design D5, D6, D2): the collection set the core keeps, the picked item the
//! Hero shows, and the album track load an open starts.

use mbv::ClientTickHost;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_net::mock_http::MockHttp;

use super::{TRACKS, music_core, send_library};

/// The collection command toggles the destination's selection (design D5,
/// D6): the seeded checked group is the only checked collection, so a press on
/// it unchecks it, the selection falls back to All, and the frame paints the
/// core's choice.
#[test]
fn a_press_on_the_checked_collection_returns_the_selection_to_all() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::SelectCollection {
            lib_idx: 0,
            group_id: "grp-hiphop".into(),
        },
    );

    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        None,
        "a press on the one checked group unchecks it and selects All"
    );
}

/// The show-detail command makes the picked item the Hero's (design D2).
#[test]
fn the_show_detail_command_shows_the_picked_album() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-ill".into(),
        },
    );

    assert_eq!(
        test.core
            .app_mut()
            .gui_library_pick_for_test(0, "grp-hiphop"),
        Some("alb-ill")
    );
}

/// The open-album command starts the album's track load when the cache does
/// not hold it (design D5): the tracks the tracklist paints arrive on the
/// drain.
#[test]
fn the_open_album_command_loads_the_albums_tracks() {
    let http = MockHttp::new();
    // One answer for the open's own track load; the pinned selection keeps
    // the launch restore's application off this destination, so no other
    // fetch runs.
    http.respond_repeated(200, TRACKS, 1);
    let (mut test, _cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::OpenAlbum {
            album_id: "alb-ill".into(),
        },
    );

    assert!(
        test.core.app_mut().album_tracks_loading_for_test("alb-ill"),
        "the album's track fetch is in flight"
    );
}
