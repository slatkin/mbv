//! The Audiobooks destination's contracts (add-gui-library-panel design D12,
//! row 9.1): the presented tabs name the Audiobookshelf book libraries and
//! never the podcast ones, and the open-destination command activates the
//! book library through the same presented-position gate every destination
//! shares.

use mbv::ClientTickHost;
use mbv_ui_model::tab_selection::TabSelection;

use super::send_library;

/// A core thread over an App whose catalog holds one Audiobookshelf book
/// library and one podcast library, with no Emby library: the Audiobooks
/// destination is the only presented one (row 9.1; the podcast destination
/// joins in row 10.1).
fn audiobooks_core() -> crate::TestCore {
    let mut test = crate::test_core(0);
    test.core.app_mut().seed_gui_abs_libraries_for_test();
    test
}

/// The presented tabs carry the book library and never the podcast library
/// (row 9.1, contract c): the Audiobooks destination joins the presented set
/// with its library's label, so the top bar paints it and the launch
/// restore's fallback reads the same set.
#[test]
fn the_presented_tabs_carry_the_book_library_and_not_the_podcast_library() {
    let mut test = audiobooks_core();

    let tabs = test.core.app_mut().gui_library_tabs();
    let labels: Vec<&str> = tabs.tabs.iter().map(|tab| tab.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Audiobooks"],
        "the book library presents a tab and the podcast library none"
    );
}

/// The open-destination command activates the book library (row 9.1): the
/// Audiobooks tab's position opens it, and the active destination's kind is
/// the Audiobooks one the frame path and the open step read.
#[test]
fn the_open_destination_command_opens_the_audiobooks_tab() {
    let mut test = audiobooks_core();

    let position = test
        .core
        .app_mut()
        .gui_library_tabs()
        .tabs
        .iter()
        .find(|tab| tab.label == "Audiobooks")
        .map(|tab| tab.position)
        .expect("the presented tabs carry the Audiobooks tab");
    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::OpenDestination { position },
    );

    let app = test.core.app_mut();
    assert_eq!(
        app.tab_for_test(),
        TabSelection::AudiobookshelfLibrary(0),
        "the Audiobooks destination is active"
    );
    assert_eq!(
        app.gui_active_destination()
            .and_then(mbv::GuiDestinationKind::library_index),
        Some(0),
        "the active destination's kind is the Audiobooks one"
    );
}
