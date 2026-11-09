//! The destination's contracts (add-gui-library-panel design D2, D4): an
//! open-destination command activates only a destination the GUI presents,
//! and the two take-focus commands move both focus authorities.

use mbv::ClientTickHost;
use mbv_emby_model::EmbyItem;
use mbv_gui::bridge::frame::focus::{GuiPanel, GuiSurface};
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

/// The take-focus command (design D4): the Library panel takes Panel focus,
/// and the next frame carries it. The Library is mounted, so the stack
/// resolves it as the focused surface.
#[test]
fn the_take_focus_command_puts_the_library_in_the_next_frame() {
    let mut test = crate::test_core(2);
    send_library(&mut test, GuiLibraryCommand::TakeFocus);

    let frame = test.next_frame();
    assert_eq!(frame.focus.panel, GuiPanel::Library);
    assert_eq!(frame.focus.surface, Some(GuiSurface::Library));
}

/// The take-queue-focus command returns Panel focus to the Queue panel on
/// both authorities (design D4): the App's stored focus, which its key gates
/// read, and the core's `FocusStack`, which the frame's focus derives from.
/// Regression for the Phase-1 gate finding: the command vocabulary had the
/// Library's take-focus alone, so Tab could never leave the Library panel.
#[test]
fn the_take_queue_focus_command_moves_both_focus_authorities_to_the_queue() {
    let mut test = crate::test_core(2);
    send_library(&mut test, GuiLibraryCommand::TakeFocus);
    send_library(&mut test, GuiLibraryCommand::TakeQueueFocus);

    let _library_frame = test.next_frame();
    let frame = test.next_frame();
    assert_eq!(frame.focus.panel, GuiPanel::Queue);
    assert_eq!(frame.focus.surface, Some(GuiSurface::Queue));
    assert_eq!(
        test.core.app_mut().gui_panel_focus(),
        mbv_ui_model::settings::PanelFocus::Queue
    );
}
