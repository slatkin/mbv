//! The Playlists screen's core state: it is its own screen, never a Settings
//! section, and it never shows together with the Settings screen.

use super::*;

/// Contract: the toggle opens the screen when closed and closes it when open.
#[test]
fn the_toggle_opens_then_closes_the_screen() {
    let mut app = make_app_stub();

    app.toggle_gui_playlists();
    assert!(app.gui_playlists_is_open());
    assert!(app.gui_playlists().is_some());

    app.toggle_gui_playlists();
    assert!(!app.gui_playlists_is_open());
    assert!(app.gui_playlists().is_none());
}

/// Contract: opening one screen closes the other, so the two never show
/// together.
#[test]
fn opening_settings_closes_the_playlists_screen_and_back() {
    let mut app = make_app_stub();
    app.open_gui_playlists();

    let mut core = crate::gui::GuiCore::new(std::mem::replace(&mut app, make_app_stub()));
    core.open_gui_settings_at(GuiSettingsSection::Services);
    app = core.into_app();
    assert!(!app.gui_playlists_is_open());

    app.open_gui_playlists();
    let core = crate::gui::GuiCore::new(app);
    assert!(core.gui_settings().is_none());
}
