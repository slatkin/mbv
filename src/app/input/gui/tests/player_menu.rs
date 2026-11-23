//! The player panel's context menu's GUI routing (gui-player-overlay-menu):
//! the right-click request opens it at the pointer, Esc closes it with no
//! toggle, and Enter and the entry press run the same auto-hide toggle and
//! close the menu with the choice.

use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, Msg, PlaybackRequest};

use crate::app::gui_client_state::{GuiOverlay, GuiOverlayProjection};
use crate::app::input::gui::GuiInput;
use crate::app::tests::make_app_stub;
use crate::gui::GuiCore;

/// One key, delivered through the GUI entry with no focused leaf.
fn gui_key(core: &mut GuiCore, key: Key) {
    core.route_gui_input(GuiInput::Key(KeyEvent::new(key, KeyModifiers::NONE)), None);
}

/// A playback request delivered through the GUI Client's entry, which runs the
/// shared App-side effect and then the Client's own half (design D4a (a)).
fn gui_request(core: &mut GuiCore, request: PlaybackRequest) {
    core.handle_gui_surface_msg(Msg::Playback(request));
}

// The player panel's right-click opens the menu at the pointer anchor the
// request carried: the frame projects the anchor, and the cursor rests on
// the menu's single entry.
#[test]
fn player_menu_right_click_opens_at_the_pointer() {
    let mut core = GuiCore::new(make_app_stub());

    gui_request(
        &mut core,
        PlaybackRequest::OpenPlayerMenu { x: 30.0, y: 40.0 },
    );

    assert!(matches!(
        core.app.gui.overlay,
        Some(GuiOverlay::PlayerMenu(_))
    ));
    let playback = core.app.queue_playback_projection();
    let Some(GuiOverlayProjection::PlayerMenu { cursor, x, y }) =
        core.gui_overlay_projection(&playback)
    else {
        panic!("the player menu projects");
    };
    assert_eq!((cursor, x, y), (0, 30.0, 40.0));
}

// Esc closes the menu and toggles nothing: a dismissal is not a choice.
#[test]
fn player_menu_esc_closes_with_no_toggle() {
    let mut core = GuiCore::new(make_app_stub());
    gui_request(
        &mut core,
        PlaybackRequest::OpenPlayerMenu { x: 10.0, y: 10.0 },
    );
    let before = core.app.gui.player_auto_hide;

    gui_key(&mut core, Key::Esc);

    assert!(core.app.gui.overlay.is_none());
    assert_eq!(core.app.gui.player_auto_hide, before);
}

// Enter on the menu toggles the persisted preference through the same
// execution the entry press applies, and the menu closes with the choice.
#[test]
fn player_menu_enter_toggles_and_closes() {
    let mut core = GuiCore::new(make_app_stub());
    gui_request(
        &mut core,
        PlaybackRequest::OpenPlayerMenu { x: 10.0, y: 10.0 },
    );
    let before = core.app.gui.player_auto_hide;

    gui_key(&mut core, Key::Enter);

    assert!(core.app.gui.overlay.is_none());
    assert_eq!(core.app.gui.player_auto_hide, !before);
}

// The entry press runs the same toggle and close as the menu's Enter, and a
// stale press after the menu closed toggles nothing.
#[test]
fn player_menu_entry_press_toggles_and_a_stale_press_changes_nothing() {
    let mut core = GuiCore::new(make_app_stub());
    gui_request(
        &mut core,
        PlaybackRequest::OpenPlayerMenu { x: 10.0, y: 10.0 },
    );
    let before = core.app.gui.player_auto_hide;

    gui_request(&mut core, PlaybackRequest::PlayerMenuSelect);
    assert!(core.app.gui.overlay.is_none());
    assert_eq!(core.app.gui.player_auto_hide, !before);

    gui_request(&mut core, PlaybackRequest::PlayerMenuSelect);
    assert_eq!(core.app.gui.player_auto_hide, !before);
}
