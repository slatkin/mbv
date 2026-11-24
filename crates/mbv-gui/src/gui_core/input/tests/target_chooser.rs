//! The playback-target chooser's GUI routing (add-tauri-gui-client row 6.4):
//! the header press and the `r` chord open it, Enter and the select request
//! send the Owner the chosen output, and the local row returns a cast output
//! to local.

use std::sync::mpsc::Receiver;

use mbv_ctrl::CtrlCmd;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, Msg, ShellRequest};

use crate::gui_core::GuiCore;
use crate::gui_core::gui_client_state::GuiOverlay;
use crate::gui_core::input::GuiInput;
use mbv_client_core::{
    make_local_daemon_app_stub_with_cmd_rx_and_compat, transport_client_capabilities,
};

/// One routed key, delivered through the GUI entry.
fn gui_key(core: &mut GuiCore, input: GuiInput) -> bool {
    core.route_gui_input(input, None)
}

/// One surface message, delivered through the GUI entry.
fn gui_msg(core: &mut GuiCore, msg: Msg) {
    core.handle_gui_surface_msg(msg);
}

/// An `App` stub whose Owner advertises the `playback-output` capability
/// and offers one cast receiver (`receiver-1`) as a playback target: the
/// `panel_targets` merge the chooser projects, with the command channel
/// that records what a selection sends (add-tauri-gui-client row 6.4).
fn cast_target_core() -> (GuiCore, Receiver<CtrlCmd>) {
    let mut compat = mbv_ctrl::CtrlCompatibility::current();
    compat.supports_playback_output = true;
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx_and_compat(Vec::new(), compat);
    app.seed_cast_receivers_for_test(vec![mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }]);
    app.rebuild_panel_targets_for_test();
    while cmd_rx.try_recv().is_ok() {}
    (GuiCore::new(app), cmd_rx)
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
    let (mut core, _cmd_rx) = cast_target_core();

    gui_msg(&mut core, Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    assert!(matches!(
        core.gui.overlay,
        Some(GuiOverlay::TargetChooser(_))
    ));

    gui_msg(&mut core, Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    assert!(core.gui.overlay.is_none());
}

// The chooser's local row returns a remote output to this machine, asked
// first: the receiver holds the stub's active slot, so the shared
// continue-playback gate raises the confirm (confirm-output-handoff
// "Selecting local playback asks while a remote output plays"), and the
// confirm's Yes sends the same hand-back the Sessions sidebar's detach
// sends.
#[test]
fn the_local_chooser_row_returns_a_cast_output_to_local() {
    let (mut core, cmd_rx) = cast_target_core();
    // The Owner's connection record reports the Cast output — the fact the
    // detach's gate and body read (the drained mirror lags it one tick);
    // the chooser's attach check still reads the mirror.
    let output = mbv_ctrl::OutputState::Cast {
        receiver_id: "receiver-1".to_string(),
        name: "Living Room".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
    };
    core.app.set_player_output_for_test(output.clone());
    core.app.seed_playback_output_for_test(Some(output));

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::SelectChooserTarget(
            mbv_ui_model::panel_targets::ChooserTargetKey::Local,
        ))),
    );

    assert!(
        output_selections(&cmd_rx).is_empty(),
        "the gate asks before anything is sent"
    );
    // The GUI drain presents the raised request as the canvas's own
    // blocking overlay.
    super::present_pending_modal(&mut core);
    assert!(matches!(core.gui.overlay, Some(GuiOverlay::Confirm(_))));
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
    );

    let selections = output_selections(&cmd_rx);
    assert_eq!(selections.len(), 1);
    assert!(matches!(selections[0], mbv_ctrl::OutputSelection::Local));
    assert!(core.gui.overlay.is_none());
}

// The GUI-only `r` chord opens the chooser (add-tauri-gui-client row 6.4):
// no TUI default binds `r`, and Esc — the overlay's own key rule — closes
// it.
#[test]
fn gui_context_r_opens_the_target_chooser() {
    let (mut core, _cmd_rx) = cast_target_core();
    core.app
        .set_client_capabilities_for_test(transport_client_capabilities());

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('r'), KeyModifiers::NONE)),
    );
    assert!(matches!(
        core.gui.overlay,
        Some(GuiOverlay::TargetChooser(_))
    ));

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
    );
    assert!(core.gui.overlay.is_none());
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
    let (mut core, cmd_rx) = cast_target_core();
    gui_msg(&mut core, Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Down, KeyModifiers::NONE)),
    );

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
    );

    assert!(
        output_selections(&cmd_rx).is_empty(),
        "the gate asks before anything is sent"
    );
    // The GUI drain presents the raised request as the canvas's own
    // blocking overlay.
    super::present_pending_modal(&mut core);
    assert!(matches!(core.gui.overlay, Some(GuiOverlay::Confirm(_))));
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
    );

    let selections = output_selections(&cmd_rx);
    assert_eq!(selections.len(), 1);
    assert!(matches!(
        selections[0],
        mbv_ctrl::OutputSelection::Cast { ref receiver_id, .. } if receiver_id == "receiver-1"
    ));
    assert!(core.gui.overlay.is_none());
}

// A chooser row's press emits the `SelectSession` request the TUI's
// Sessions sidebar sends for its selected row: the dispatch resolves the
// stable key, the shared continue-playback gate asks first (the stub owner
// plays), and the confirm's Yes selects the output and closes the chooser
// (confirm-output-handoff).
#[test]
fn select_session_request_selects_the_output_and_closes() {
    let (mut core, cmd_rx) = cast_target_core();
    gui_msg(&mut core, Msg::Shell(Box::new(ShellRequest::OpenSessions)));

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::SelectSession(
            mbv_ui_model::panel_targets::SessionTargetKey::Cast("receiver-1".to_string()),
        ))),
    );

    assert!(
        output_selections(&cmd_rx).is_empty(),
        "the gate asks before anything is sent"
    );
    super::present_pending_modal(&mut core);
    assert!(matches!(core.gui.overlay, Some(GuiOverlay::Confirm(_))));
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
    );

    let selections = output_selections(&cmd_rx);
    assert_eq!(selections.len(), 1);
    assert!(core.gui.overlay.is_none());
}

// Opening the chooser refreshes the list it shows: the session load and the
// cast browse start on the open, as the TUI's F3 open starts them. Closing
// starts nothing.
#[test]
fn opening_the_chooser_refreshes_the_targets_and_closing_does_not() {
    let (mut core, _cmd_rx) = cast_target_core();
    assert_eq!(core.app_mut().cast_discovery_starts_for_test(), 0);

    gui_msg(&mut core, Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    assert_eq!(
        core.app_mut().cast_discovery_starts_for_test(),
        1,
        "the open must start one cast discovery"
    );
    assert!(
        core.app_mut().sessions_load_started_for_test(),
        "the open must start the sessions load"
    );

    gui_msg(&mut core, Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    assert!(core.gui.overlay.is_none());
    assert_eq!(
        core.app_mut().cast_discovery_starts_for_test(),
        1,
        "the close must start no discovery"
    );
}

// A Refresh press starts a refresh and leaves the chooser open; a second
// press while the browse is still in flight stacks no work.
#[test]
fn a_refresh_while_in_flight_starts_no_second_discovery_and_keeps_the_chooser() {
    let (mut core, _cmd_rx) = cast_target_core();
    gui_msg(&mut core, Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    assert!(core.app_mut().cast_discovery_loading_for_test());

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RefreshSessions)),
    );
    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RefreshSessions)),
    );

    assert_eq!(
        core.app_mut().cast_discovery_starts_for_test(),
        1,
        "the in-flight guard must refuse the second browse"
    );
    assert!(matches!(
        core.gui.overlay,
        Some(GuiOverlay::TargetChooser(_))
    ));
}

// The sessions half holds its own in-flight guard: a refresh while the
// `/Sessions` load runs spawns no second one, so repeated presses stack no
// work.
#[test]
fn a_second_refresh_while_the_sessions_load_runs_spawns_no_second_load() {
    let (mut core, _cmd_rx) = cast_target_core();
    gui_msg(&mut core, Msg::Shell(Box::new(ShellRequest::OpenSessions)));
    assert!(core.app_mut().sessions_load_started_for_test());

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RefreshSessions)),
    );

    assert!(
        core.app_mut().sessions_load_started_for_test(),
        "the guard refuses the second load and leaves the first in flight"
    );
}
