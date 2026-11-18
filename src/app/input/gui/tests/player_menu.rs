//! The player panel's context menu's GUI routing (gui-player-overlay-menu):
//! the right-click request opens it at the pointer, Esc closes it with no
//! toggle, and Enter and the entry press run the same auto-hide toggle and
//! close the menu with the choice.

use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, PlaybackRequest};

use crate::app::gui_client_state::{GuiOverlay, GuiOverlayProjection};
use crate::app::input::gui::GuiInput;
use crate::app::tests::make_app_stub;

// The player panel's right-click opens the menu at the pointer anchor the
// request carried: the frame projects the anchor, and the cursor rests on
// the menu's single entry.
#[test]
fn player_menu_right_click_opens_at_the_pointer() {
    let mut app = make_app_stub();

    app.handle_playback_request(&PlaybackRequest::OpenPlayerMenu { x: 30.0, y: 40.0 });

    assert!(matches!(app.gui_overlay, Some(GuiOverlay::PlayerMenu(_))));
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
    app.handle_playback_request(&PlaybackRequest::OpenPlayerMenu { x: 10.0, y: 10.0 });
    let before = app.gui_player_auto_hide;

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
        None,
    );

    assert!(app.gui_overlay.is_none());
    assert_eq!(app.gui_player_auto_hide, before);
}

// Enter on the menu toggles the persisted preference through the same
// execution the entry press applies, and the menu closes with the choice.
#[test]
fn player_menu_enter_toggles_and_closes() {
    let mut app = make_app_stub();
    app.handle_playback_request(&PlaybackRequest::OpenPlayerMenu { x: 10.0, y: 10.0 });
    let before = app.gui_player_auto_hide;

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
        None,
    );

    assert!(app.gui_overlay.is_none());
    assert_eq!(app.gui_player_auto_hide, !before);
}

// The entry press runs the same toggle and close as the menu's Enter, and a
// stale press after the menu closed toggles nothing.
#[test]
fn player_menu_entry_press_toggles_and_a_stale_press_changes_nothing() {
    let mut app = make_app_stub();
    app.handle_playback_request(&PlaybackRequest::OpenPlayerMenu { x: 10.0, y: 10.0 });
    let before = app.gui_player_auto_hide;

    app.handle_playback_request(&PlaybackRequest::PlayerMenuSelect);
    assert!(app.gui_overlay.is_none());
    assert_eq!(app.gui_player_auto_hide, !before);

    app.handle_playback_request(&PlaybackRequest::PlayerMenuSelect);
    assert_eq!(app.gui_player_auto_hide, !before);
}
