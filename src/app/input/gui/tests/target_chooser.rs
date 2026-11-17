//! The playback-target chooser's GUI routing (add-tauri-gui-client row 6.4):
//! the header press and the `r` chord open it, Enter and the select request
//! send the Owner the chosen output, and the local row returns a cast output
//! to local.

use std::sync::mpsc::Receiver;

use mbv_ctrl::CtrlCmd;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, Msg, ShellRequest};

use crate::app::App;
use crate::app::gui_client_state::GuiOverlay;
use crate::app::input::gui::GuiInput;
use crate::app::tests::make_local_daemon_app_stub_with_cmd_rx_and_compat;

/// An `App` stub whose Owner advertises the `playback-output` capability
/// and offers one cast receiver (`receiver-1`) as a playback target: the
/// `panel_targets` merge the chooser projects, with the command channel
/// that records what a selection sends (add-tauri-gui-client row 6.4).
fn cast_target_app() -> (App, Receiver<CtrlCmd>) {
    let mut compat = mbv_ctrl::CtrlCompatibility::current();
    compat.supports_playback_output = true;
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx_and_compat(Vec::new(), compat);
    app.cast_receivers = vec![mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }];
    app.rebuild_panel_targets();
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

/// The `SelectPlaybackOutput` selections the Owner received, in arrival
/// order.
fn output_selections(cmd_rx: &Receiver<CtrlCmd>) -> Vec<mbv_ctrl::OutputSelection> {
    cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CtrlCmd::SelectPlaybackOutput(selection) => Some(selection),
            _ => None,
        })
        .collect()
}

// The header's playback-target press emits the same opener the TUI's
// indicator sends the Sessions sidebar (add-tauri-gui-client row 6.4); the
// GUI's counterpart is the Client-core chooser overlay, and the press
// toggles it as F3 toggles the sidebar.
#[test]
fn open_sessions_request_toggles_the_target_chooser() {
    let (mut app, _cmd_rx) = cast_target_app();

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    assert!(matches!(
        app.gui_overlay,
        Some(GuiOverlay::TargetChooser(_))
    ));

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    assert!(app.gui_overlay.is_none());
}

// The chooser's local row returns a remote output to this machine, asked
// first: the receiver holds the stub's active slot, so the shared
// continue-playback gate raises the confirm (confirm-output-handoff
// "Selecting local playback asks while a remote output plays"), and the
// confirm's Yes sends the same hand-back the Sessions sidebar's detach
// sends.
#[test]
fn the_local_chooser_row_returns_a_cast_output_to_local() {
    let (mut app, cmd_rx) = cast_target_app();
    // The Owner's connection record reports the Cast output — the fact the
    // detach's gate and body read (the drained mirror lags it one tick);
    // the chooser's attach check still reads the mirror.
    let output = mbv_ctrl::OutputState::Cast {
        receiver_id: "receiver-1".to_string(),
        name: "Living Room".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
    };
    app.player
        .remote()
        .update_playback_output_for_test(output.clone());
    app.playback_output = Some(output);

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::SelectChooserTarget(
        mbv_ui_model::panel_targets::ChooserTargetKey::Local,
    ))));

    assert!(
        output_selections(&cmd_rx).is_empty(),
        "the gate asks before anything is sent"
    );
    // The GUI drain presents the raised request as the canvas's own
    // blocking overlay.
    super::present_pending_modal(&mut app);
    assert!(matches!(app.gui_overlay, Some(GuiOverlay::Confirm(_))));
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
        None,
    );

    let selections = output_selections(&cmd_rx);
    assert_eq!(selections.len(), 1);
    assert!(matches!(selections[0], mbv_ctrl::OutputSelection::Local));
    assert!(app.gui_overlay.is_none());
}

// The GUI-only `r` chord opens the chooser (add-tauri-gui-client row 6.4):
// no TUI default binds `r`, and Esc — the overlay's own key rule — closes
// it.
#[test]
fn gui_context_r_opens_the_target_chooser() {
    let (mut app, _cmd_rx) = cast_target_app();
    app.enable_gui_client();

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('r'), KeyModifiers::NONE)),
        None,
    );
    assert!(matches!(
        app.gui_overlay,
        Some(GuiOverlay::TargetChooser(_))
    ));

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
        None,
    );
    assert!(app.gui_overlay.is_none());
}

// A chooser Enter selects the cursor choice's output through the same
// dispatch the TUI's Sessions sidebar applies: the cast receiver's stable
// key resolves against the target snapshot, and the shared continue-playback
// gate asks first — the stub owner plays, so Enter raises the confirm, and
// the confirm's Yes sends the `SelectPlaybackOutput` the F3 selection sends
// (confirm-output-handoff). The cursor starts on the local row, so the test
// moves it to the cast receiver first.
#[test]
fn target_chooser_enter_selects_the_cursor_choice() {
    let (mut app, cmd_rx) = cast_target_app();
    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Down, KeyModifiers::NONE)),
        None,
    );

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
        None,
    );

    assert!(
        output_selections(&cmd_rx).is_empty(),
        "the gate asks before anything is sent"
    );
    // The GUI drain presents the raised request as the canvas's own
    // blocking overlay.
    super::present_pending_modal(&mut app);
    assert!(matches!(app.gui_overlay, Some(GuiOverlay::Confirm(_))));
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
        None,
    );

    let selections = output_selections(&cmd_rx);
    assert_eq!(selections.len(), 1);
    assert!(matches!(
        selections[0],
        mbv_ctrl::OutputSelection::Cast { ref receiver_id, .. } if receiver_id == "receiver-1"
    ));
    assert!(app.gui_overlay.is_none());
}

// A chooser row's press emits the `SelectSession` request the TUI's
// Sessions sidebar sends for its selected row: the dispatch resolves the
// stable key, the shared continue-playback gate asks first (the stub owner
// plays), and the confirm's Yes selects the output and closes the chooser
// (confirm-output-handoff).
#[test]
fn select_session_request_selects_the_output_and_closes() {
    let (mut app, cmd_rx) = cast_target_app();
    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::OpenSessions)));

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::SelectSession(
        mbv_ui_model::panel_targets::SessionTargetKey::Cast("receiver-1".to_string()),
    ))));

    assert!(
        output_selections(&cmd_rx).is_empty(),
        "the gate asks before anything is sent"
    );
    super::present_pending_modal(&mut app);
    assert!(matches!(app.gui_overlay, Some(GuiOverlay::Confirm(_))));
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
        None,
    );

    let selections = output_selections(&cmd_rx);
    assert_eq!(selections.len(), 1);
    assert!(app.gui_overlay.is_none());
}
