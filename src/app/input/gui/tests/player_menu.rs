//! The player panel's context menu's GUI routing (gui-player-overlay-menu):
//! the right-click request opens it at the pointer, Esc closes it with no
//! toggle, and Enter and the entry press run the same auto-hide toggle and
//! close the menu with the choice.

use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, Msg, PlaybackRequest};

use crate::app::App;
use crate::app::gui_client_state::{GuiOverlay, GuiOverlayProjection};
use crate::app::input::gui::GuiInput;
use crate::app::input::gui::with_gui_core;
use crate::app::tests::make_app_stub;

/// One key, delivered through the GUI entry on `app`'s own core with no
/// focused leaf.
fn gui_key(app: &mut App, key: Key) {
    with_gui_core(app, |core| {
        core.route_gui_input(GuiInput::Key(KeyEvent::new(key, KeyModifiers::NONE)), None);
    });
}

/// A playback request delivered through the GUI Client's entry, which runs the
/// shared App-side effect and then the Client's own half (design D4a (a)).
fn gui_request(app: &mut App, request: PlaybackRequest) {
    with_gui_core(app, |core| {
        core.handle_gui_surface_msg(Msg::Playback(request));
    });
}

// The player panel's right-click opens the menu at the pointer anchor the
// request carried: the frame projects the anchor, and the cursor rests on
// the menu's single entry.
#[test]
fn player_menu_right_click_opens_at_the_pointer() {
    let mut app = make_app_stub();

    gui_request(
        &mut app,
        PlaybackRequest::OpenPlayerMenu { x: 30.0, y: 40.0 },
    );

    assert!(matches!(app.gui.overlay, Some(GuiOverlay::PlayerMenu(_))));
    let playback = app.queue_playback_projection();
    let Some(GuiOverlayProjection::PlayerMenu { cursor, x, y }) =
        app.gui_overlay_projection(&playback)
    else {
        panic!("the player menu projects");
    };
    assert_eq!((cursor, x, y), (0, 30.0, 40.0));
}

// Esc closes the menu and toggles nothing: a dismissal is not a choice.
#[test]
fn player_menu_esc_closes_with_no_toggle() {
    let mut app = make_app_stub();
    gui_request(
        &mut app,
        PlaybackRequest::OpenPlayerMenu { x: 10.0, y: 10.0 },
    );
    let before = app.gui.player_auto_hide;

    gui_key(&mut app, Key::Esc);

    assert!(app.gui.overlay.is_none());
    assert_eq!(app.gui.player_auto_hide, before);
}

// Enter on the menu toggles the persisted preference through the same
// execution the entry press applies, and the menu closes with the choice.
#[test]
fn player_menu_enter_toggles_and_closes() {
    let mut app = make_app_stub();
    gui_request(
        &mut app,
        PlaybackRequest::OpenPlayerMenu { x: 10.0, y: 10.0 },
    );
    let before = app.gui.player_auto_hide;

    gui_key(&mut app, Key::Enter);

    assert!(app.gui.overlay.is_none());
    assert_eq!(app.gui.player_auto_hide, !before);
}

// The entry press runs the same toggle and close as the menu's Enter, and a
// stale press after the menu closed toggles nothing.
#[test]
fn player_menu_entry_press_toggles_and_a_stale_press_changes_nothing() {
    let mut app = make_app_stub();
    gui_request(
        &mut app,
        PlaybackRequest::OpenPlayerMenu { x: 10.0, y: 10.0 },
    );
    let before = app.gui.player_auto_hide;

    gui_request(&mut app, PlaybackRequest::PlayerMenuSelect);
    assert!(app.gui.overlay.is_none());
    assert_eq!(app.gui.player_auto_hide, !before);

    gui_request(&mut app, PlaybackRequest::PlayerMenuSelect);
    assert_eq!(app.gui.player_auto_hide, !before);
}
