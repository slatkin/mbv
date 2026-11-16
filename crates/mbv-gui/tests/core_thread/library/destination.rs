//! The destination's contracts (add-gui-library-panel design D2): an
//! open-destination command activates only a destination the GUI presents.

use mbv::ClientTickHost;
use mbv_emby_model::EmbyItem;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::tab_selection::TabSelection;

use super::{music_core, send_library};

/// An open-destination command makes the destination active (design D2): Home
/// holds position 0 and the first library position 1.
#[test]
fn the_open_destination_command_opens_the_music_tab() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 0 },
    );
    assert_eq!(test.core.app_mut().tab_for_test(), TabSelection::Home);

    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 1 },
    );
    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::EmbyLibrary(0)
    );
}

/// An open-destination command for a position no presented destination names
/// is a no-op (design D2: the top bar carries only the presented tabs, and a
/// destination the GUI does not present is absent — a digit key for one opens
/// nothing). Regression for the Phase-1 gate finding: the command applied the
/// raw position, so a position past the presented tabs activated the
/// unpresented library. Row 8.1 presents every plain Emby library, so the
/// unpresented case is now a position past the tab row's end.
#[test]
fn an_open_destination_command_for_an_unpresented_destination_is_a_no_op() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = music_core(&http);
    test.core
        .app_mut()
        .push_gui_library_tab_for_test(EmbyItem::folder(
            "lib-movies".into(),
            "Movies".into(),
            "movies".into(),
        ));

    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 0 },
    );
    // Position 2 is the Movies tab's position (the presented set now names
    // it, row 8.1), and position 3 is past every tab.
    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 2 },
    );
    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::EmbyLibrary(1),
        "the Movies destination presents and opens"
    );

    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 3 },
    );
    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::EmbyLibrary(1),
        "a position past the presented tabs opens nothing"
    );
}
