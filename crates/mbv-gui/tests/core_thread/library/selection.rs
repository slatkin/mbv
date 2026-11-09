//! The destination's stored selection and detail (add-gui-library-panel
//! design D5, D6, D2): the collection set the core keeps, the picked item the
//! Hero shows, and the album track load an open starts.

use mbv::ClientTickHost;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_net::mock_http::MockHttp;

use super::{ALBUMS, TRACKS, music_core, send_library};

/// The select-collection command stores the destination's one selected group
/// (design D5, D6): the core keeps it, so the frame's pills paint the core's
/// choice.
#[test]
fn the_select_collection_command_stores_the_selected_group() {
    let http = MockHttp::new();
    http.respond(200, ALBUMS);
    let (mut test, _cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::SelectCollection {
            lib_idx: 0,
            group_id: "grp-soul".into(),
        },
    );

    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        Some("grp-soul")
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
            .gui_library_detail_for_test()
            .map(|item| item.id.as_str()),
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
