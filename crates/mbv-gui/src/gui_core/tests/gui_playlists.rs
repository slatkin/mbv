//! The Playlists screen's core state: it is its own screen, never a Settings
//! section, and it never shows together with the Settings screen.

use super::*;
use mbv_ui_model::overlay;

/// Contract: the toggle opens the screen when closed and closes it when open.
#[test]
fn the_toggle_opens_then_closes_the_screen() {
    let mut core = crate::gui_core::GuiCore::new(make_app_stub());

    core.toggle_gui_playlists();
    assert!(core.gui_playlists_is_open());
    assert!(core.gui_playlists().is_some());

    core.toggle_gui_playlists();
    assert!(!core.gui_playlists_is_open());
    assert!(core.gui_playlists().is_none());
}

/// Contract: opening one screen closes the other, so the two never show
/// together.
#[test]
fn opening_settings_closes_the_playlists_screen_and_back() {
    let mut core = crate::gui_core::GuiCore::new(make_app_stub());
    core.open_gui_playlists();

    core.apply_gui_settings_overlay(&overlay::OverlayRequest::OpenSidebar(
        overlay::SidebarId::Settings,
    ));
    assert!(!core.gui_playlists_is_open());

    core.open_gui_playlists();
    assert!(core.gui_settings().is_none());
}
