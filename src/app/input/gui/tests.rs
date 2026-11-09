use std::sync::mpsc::Receiver;

use mbv_ctrl::{CtrlCmd, WireCommand};
use mbv_ui_model::context_menu::ContextAction;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, Msg, PlaybackRequest, ShellRequest, TrackChipKind};

use crate::app::App;
use crate::app::gui_client_state::{GuiOverlay, GuiOverlayProjection};
use crate::app::input::gui::GuiInput;
use crate::app::tests::{
    QueueViewTestExt, make_app_stub, make_items, make_local_daemon_app_stub_with_cmd_rx_and_compat,
    make_remote_app_stub_with_cmd_rx,
};

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

/// The `Seek` commands the Owner received, in arrival order.
fn seek_commands(cmd_rx: &Receiver<CtrlCmd>) -> Vec<f64> {
    cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::Seek(secs)) => Some(secs),
            _ => None,
        })
        .collect()
}

/// The slot id of every unified play-slot command the owner received, in
/// arrival order.
fn played_slot_ids(cmd_rx: &Receiver<CtrlCmd>) -> Vec<u64> {
    cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CtrlCmd::UnifiedQueuePlaySlot { slot_id, .. } => Some(slot_id),
            _ => None,
        })
        .collect()
}

// U17 review finding 2: a GUI single click on a queue row runs the TUI single
// click's App effects, so the Queue panel takes focus.
#[test]
fn queue_row_click_focuses_the_queue_panel_review_u17_2() {
    let mut app = make_app_stub();
    app.playback_queue_mut().adopt_items(make_items(2), 0);
    app.set_panel_focus(PanelFocus::Library);
    let row = app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::QueueRowClick {
        slot_id: Some(row),
    })));

    assert_eq!(app.effective_panel_focus(), PanelFocus::Queue);
}

// U17 review finding 2: a GUI double click on a queue row plays that row
// through the TUI double click's path, so the owner receives the play of the
// row's slot.
#[test]
fn queue_row_activate_plays_the_clicked_row_review_u17_2() {
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    // The direct-remote stub displays its Peer queue, so the rows go there.
    app.playback_queue_mut().adopt_items(make_items(3), 0);
    let row = app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::QueueRowActivate {
        slot_id: Some(row),
    })));

    assert_eq!(played_slot_ids(&cmd_rx), vec![row.raw()]);
}

// The GUI-only key context (add-tauri-gui-client row 6.2): the canvas's `z`
// collapses the panel, and the same chord stays `cycle_subtitle`'s in the TUI
// — the routing matrix pins the TUI side, this pins the GUI side.
#[test]
fn gui_context_z_toggles_the_panel_collapse() {
    let mut app = make_app_stub();

    assert!(!app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('z'), KeyModifiers::NONE)),
        None
    ));
    assert!(app.panel_collapsed);

    assert!(!app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('z'), KeyModifiers::NONE)),
        None
    ));
    assert!(!app.panel_collapsed);
}

// The GUI-only key context's `q` folds the queue; the TUI's `quit` on the
// same chord is untouched there (the routing matrix pins it).
#[test]
fn gui_context_q_toggles_the_queue_fold() {
    let mut app = make_app_stub();

    assert!(!app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('q'), KeyModifiers::NONE)),
        None
    ));
    assert!(app.queue_folded);
}

// The GUI-only key context's `c` cycles the subtitle track, the chord the
// TUI binds to the clear-queue prompt. With the stub's idle player the
// dispatch lands in the subtitle-mode cycle, so the config value is the
// observable.
#[test]
fn gui_context_c_cycles_the_subtitle_track() {
    let mut app = make_app_stub();
    // The stub player starts active; the subtitle cycle's active path needs
    // tracks the stub does not carry, so the test drives the idle path the
    // mode cycle lives on.
    app.player.update_status(|status| status.active = false);
    let before = app
        .config
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .subtitle_mode
        .clone();

    assert!(!app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('c'), KeyModifiers::NONE)),
        None
    ));

    let after = app
        .config
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .subtitle_mode
        .clone();
    assert_ne!(after, before);
}

// The queue filter's routing (add-tauri-gui-client row 6.2): the GUI-only
// `/` focuses the text entry, the characters append while it holds the
// keyboard, Backspace pops, and Esc closes and clears.
#[test]
fn gui_queue_filter_opens_types_and_closes() {
    let mut app = make_app_stub();
    let key = |c: char| GuiInput::Key(KeyEvent::new(Key::Char(c), KeyModifiers::NONE));

    app.route_gui_input(key('/'), None);
    assert!(app.queue_filter_active);

    app.route_gui_input(key('a'), None);
    app.route_gui_input(key('b'), None);
    assert_eq!(app.queue_filter, "ab");

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Backspace, KeyModifiers::NONE)),
        None,
    );
    assert_eq!(app.queue_filter, "a");

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
        None,
    );
    assert!(!app.queue_filter_active);
    assert_eq!(app.queue_filter, "");
}

// The GUI's row menu opens from the row's context-menu request — the
// keyboard's `.` and the right-click resolve through the same request — with
// the canvas's entries and their key hints.
#[test]
fn row_context_menu_request_opens_the_row_menu_with_hints() {
    let mut app = make_app_stub();
    app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = app
        .displayed_queue()
        .slot_id_at(0)
        .expect("row 0 has a slot");

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
        mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        None,
    ))));

    let Some(GuiOverlay::RowMenu(menu)) = &app.gui_overlay else {
        panic!("the row menu opened");
    };
    assert_eq!(menu.cursor, 0);
    let hints: Vec<Option<&str>> = menu.entries.iter().map(|entry| entry.key_hint).collect();
    assert_eq!(
        hints,
        vec![
            Some("Enter"),
            Some("n"),
            Some("w"),
            Some("g"),
            Some("dd"),
            Some("c")
        ]
    );
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert_eq!(
        labels,
        vec![
            "Play now",
            "Play next",
            "Mark watched",
            "Go to episode",
            "Remove from queue",
            "Clear queue"
        ]
    );
}

// The destructive flag is GUI-only and derives from the entry's action
// (design DR6): the row menu projects its remove entry destructive and no
// other entry, so the front end paints the canvas's tone from the flag and
// never from a label or a position.
#[test]
fn the_row_menu_projects_only_the_remove_entry_as_destructive() {
    let mut app = make_app_stub();
    app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = app
        .displayed_queue()
        .slot_id_at(0)
        .expect("row 0 has a slot");
    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
        mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        None,
    ))));

    let playback = app.queue_playback_projection();
    let Some(GuiOverlayProjection::RowMenu { entries, .. }) = app.gui_overlay_projection(&playback)
    else {
        panic!("the row menu projects");
    };
    let destructive: Vec<bool> = entries.iter().map(|entry| entry.destructive).collect();
    assert_eq!(destructive, [false, false, false, false, true, true]);
}

// Enter on the open row menu executes the cursor entry and closes the menu:
// the canvas's Play now plays the retained row (split-queue-cursor-
// ownership D2), so the owner receives that slot's play. The menu opens on
// the row after the playhead: the playing row's own Play now is a
// restart-seek, not a slot play.
#[test]
fn row_menu_enter_plays_the_cursor_row() {
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    // The direct-remote stub displays its Peer queue, so the rows go there.
    app.playback_queue_mut().adopt_items(make_items(3), 0);
    let row = app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");
    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
        mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        None,
    ))));

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
        None,
    );

    assert_eq!(played_slot_ids(&cmd_rx), vec![row.raw()]);
    assert!(app.gui_overlay.is_none());
}

// The canvas's two-stroke `dd` hint on Remove: the first `d` arms, the
// second removes the retained row, so the owner receives that slot's
// remove. The menu opens on a row below the playhead, so the remove is the
// plain slot remove.
#[test]
fn row_menu_two_stroke_dd_removes_the_row() {
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");
    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
        mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        None,
    ))));

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('d'), KeyModifiers::NONE)),
        None,
    );
    // The first stroke arms only.
    assert!(app.gui_overlay.is_some());
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('d'), KeyModifiers::NONE)),
        None,
    );

    let removed: Vec<u64> = cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CtrlCmd::UnifiedQueueRemoveSlot { slot_id, .. } => Some(slot_id),
            _ => None,
        })
        .collect();
    assert_eq!(removed, vec![row.raw()]);
    assert!(app.gui_overlay.is_none());
}

// The open overlay owns the keyboard while it is up: a chord its rules do
// not bind is dropped — the panel beneath it sees nothing — and Esc
// dismisses.
#[test]
fn open_overlay_owns_the_keyboard() {
    let mut app = make_app_stub();
    app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = app
        .displayed_queue()
        .slot_id_at(0)
        .expect("row 0 has a slot");
    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
        mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        None,
    ))));

    // The GUI-only collapse chord is a menu-owned chord while the menu is
    // open: it drops, and the panel does not collapse.
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('z'), KeyModifiers::NONE)),
        None,
    );
    assert!(!app.panel_collapsed);
    assert!(app.gui_overlay.is_some());

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
        None,
    );
    assert!(app.gui_overlay.is_none());
}

// The track popover's keyboard choice sends the Owner the chosen track: Down
// past the Off entry, Enter on the track, and the popover closes.
#[test]
fn track_popover_choose_sends_the_track_switch() {
    let _state_dir = crate::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    app.player.update_status(|status| {
        status.active = true;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });

    app.handle_playback_request(&PlaybackRequest::OpenTrackPopover(TrackChipKind::Subtitles));
    let Some(GuiOverlay::TrackPopover(popover)) = &app.gui_overlay else {
        panic!("the popover opened over the chip's choices");
    };
    assert_eq!(popover.cursor, 0, "the cursor starts on the chosen Off");

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Down, KeyModifiers::NONE)),
        None,
    );
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
        None,
    );

    let switched: Vec<i64> = cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CtrlCmd::PlayerCmd(WireCommand::SetSub(id)) => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(switched, vec![2]);
    assert!(app.gui_overlay.is_none());
}

// Unit 6.2 review finding (P2): a track popover left open when the local
// player goes inactive has no chip source, so the frame projects no overlay
// for it. The held state drops with its source before the snapshot reads
// the overlay focus — otherwise the global gates stay closed and the
// overlay's key rules swallow input the window does not paint.
#[test]
fn track_popover_drops_when_its_chips_disappear_review_u62_p2() {
    let mut app = make_app_stub();
    app.player.update_status(|status| {
        status.active = true;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });
    app.handle_playback_request(&PlaybackRequest::OpenTrackPopover(TrackChipKind::Subtitles));
    assert!(app.gui_overlay.is_some());

    // Playback ends while the popover is up (the queue exhausts, or the
    // Owner stops from another client): the chips' source is gone.
    app.player.update_status(|status| status.active = false);

    // The next chord reconciles the orphaned popover before the snapshot,
    // so it routes to its binding instead of being consumed by the open
    // overlay's key rules.
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('z'), KeyModifiers::NONE)),
        None,
    );

    assert!(app.gui_overlay.is_none());
    assert!(app.panel_collapsed, "the chord routed to its binding");
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

// The chooser's local row returns a remote output to this machine: the same
// hand-back the Sessions sidebar's detach sends, reached from the chooser.
#[test]
fn the_local_chooser_row_returns_a_cast_output_to_local() {
    let (mut app, cmd_rx) = cast_target_app();
    app.playback_output = Some(mbv_ctrl::OutputState::Cast {
        receiver_id: "receiver-1".to_string(),
        name: "Living Room".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
    });

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::SelectChooserTarget(
        mbv_ui_model::panel_targets::ChooserTargetKey::Local,
    ))));

    let selections: Vec<_> = cmd_rx
        .try_iter()
        .filter(|cmd| matches!(cmd, CtrlCmd::SelectPlaybackOutput(_)))
        .collect();
    assert!(matches!(
        selections.as_slice(),
        [CtrlCmd::SelectPlaybackOutput(
            mbv_ctrl::OutputSelection::Local
        )]
    ));
}

// The GUI-only `r` chord opens the chooser (add-tauri-gui-client row 6.4):
// no TUI default binds `r`, and Esc — the overlay's own key rule — closes
// it.
#[test]
fn gui_context_r_opens_the_target_chooser() {
    let (mut app, _cmd_rx) = cast_target_app();

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
// key resolves against the target snapshot and the Owner receives the
// `SelectPlaybackOutput` the F3 selection sends (add-tauri-gui-client row
// 6.4). The cursor starts on the local row, so the test moves it to the cast
// receiver first.
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
// stable key, selects the output, and closes the chooser
// (add-tauri-gui-client row 6.4).
#[test]
fn select_session_request_selects_the_output_and_closes() {
    let (mut app, cmd_rx) = cast_target_app();
    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::OpenSessions)));

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::SelectSession(
        mbv_ui_model::panel_targets::SessionTargetKey::Cast("receiver-1".to_string()),
    ))));

    let selections = output_selections(&cmd_rx);
    assert_eq!(selections.len(), 1);
    assert!(app.gui_overlay.is_none());
}

// The canvas's back and forward controls emit the deltas the
// `seek_back_10` and `seek_forward_30` chords carry (add-tauri-gui-client
// row 6.4, the "back 10 s, forward 30 s" spec): the request's dispatch
// and the chords' dispatch send the same command. The TUI's `<`/`>` keep
// their ±5 s.
#[test]
fn seek_relative_request_matches_the_seek_chords() {
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());

    app.handle_playback_request(&PlaybackRequest::SeekRelative(-10.0));
    app.handle_playback_request(&PlaybackRequest::SeekRelative(30.0));
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char('['), KeyModifiers::NONE)),
        None,
    );
    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Char(']'), KeyModifiers::NONE)),
        None,
    );

    assert_eq!(seek_commands(&cmd_rx), vec![-10.0, 30.0, -10.0, 30.0]);
}

// The track chips' popover chords (add-tauri-gui-client row 6.4, the
// "every action is reachable from the keyboard" spec): `a` toggles the
// audio popover and `s` the subtitle popover — the same dispatch the
// chips' `OpenTrackPopover` press routes — and Esc, the overlay's own
// key rule, closes. While a popover is open it owns the keyboard, as the
// chooser's open state does.
#[test]
fn gui_context_popover_chords_toggle_the_matching_popover() {
    let mut app = make_app_stub();
    app.player.update_status(|status| {
        status.active = true;
        status.audio_tracks = vec![(1, "Stereo".to_string())];
        status.audio_id = 1;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });
    let key = |c: char| GuiInput::Key(KeyEvent::new(Key::Char(c), KeyModifiers::NONE));
    let open_chip = |app: &App| match &app.gui_overlay {
        Some(GuiOverlay::TrackPopover(popover)) => Some(popover.chip),
        _ => None,
    };

    app.route_gui_input(key('a'), None);
    assert_eq!(open_chip(&app), Some(TrackChipKind::Audio));

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
        None,
    );
    assert!(app.gui_overlay.is_none());

    app.route_gui_input(key('s'), None);
    assert_eq!(open_chip(&app), Some(TrackChipKind::Subtitles));
}

// The GUI's QueueIntent::Clear runs the TUI's own App action: the `c`
// chord's request, which raises the shared clear-queue confirmation the
// GUI now presents as its own overlay (add-gui-library-panel queue fixes —
// the prompt used to be a documented no-op, so the GUI had no clear at
// all). Enter on the overlay answers the confirm, and the owner receives
// the answered clear op the TUI's confirm sends.
#[test]
fn queue_intent_clear_opens_the_confirm_overlay_and_enter_clears_the_queue() {
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    app.enable_gui_client();
    app.playback_queue_mut().adopt_items(make_items(2), 0);

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::QueueIntent(
        mbv_ui_msg::QueueIntent::Clear,
    ))));

    let playback = app.queue_playback_projection();
    let Some(GuiOverlayProjection::Confirm { message, buttons }) =
        app.gui_overlay_projection(&playback)
    else {
        panic!("the clear confirm presented");
    };
    assert_eq!(message, "Clear the queue?");
    assert_eq!(buttons.len(), 2);

    app.route_gui_input(
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
        None,
    );

    assert!(app.gui_overlay.is_none());
    let cleared = cmd_rx.try_recv().is_ok_and(|cmd| {
        matches!(
            cmd,
            CtrlCmd::UnifiedQueueClearOp { .. } | CtrlCmd::UnifiedQueueClear
        )
    });
    assert!(cleared, "the owner received the clear op");
}

// The row menu's Clear queue entry raises the same confirm the `c` chord
// does, through the shared `ContextAction::ClearQueue` execution — one App
// action, no GUI-only route.
#[test]
fn row_menu_clear_queue_entry_raises_the_clear_confirm() {
    let mut app = make_app_stub();
    app.enable_gui_client();
    app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = app
        .displayed_queue()
        .slot_id_at(0)
        .expect("row 0 has a slot");

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
        mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        None,
    ))));
    let index = match &app.gui_overlay {
        Some(GuiOverlay::RowMenu(menu)) => menu
            .entries
            .iter()
            .position(|entry| matches!(entry.action, Some(ContextAction::ClearQueue)))
            .expect("the clear entry listed"),
        _ => panic!("the row menu did not open"),
    };

    app.select_gui_row_menu_entry(index);

    match &app.gui_overlay {
        Some(GuiOverlay::Confirm(modal)) => {
            assert_eq!(modal.message, "Clear the queue?");
        }
        _ => panic!("the clear confirm did not present"),
    }
}

// A multi-selection's row menu is the shared selection menu the TUI's
// right-click builds, so its bulk Remove names every marked slot — the
// GUI's own per-row builder used to drop all but the first (the
// select-and-delete finding). Executing it sends the owner one RemoveSlots
// edit carrying exactly those slots.
#[test]
fn multi_selection_row_menu_removes_exactly_the_selected_slots() {
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    app.enable_gui_client();
    app.playback_queue_mut().adopt_items(make_items(3), 0);
    // The stub's player reports slot 0 as the now-playing row, whose removal
    // owns its own confirmation; the selection under test is the two rows
    // behind it, so the edit goes out without a prompt.
    let slots: Vec<mbv_queue::QueueSlotId> = (1..3)
        .filter_map(|index| app.displayed_queue().slot_id_at(index))
        .collect();

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
        mbv_ui_model::context_menu::ContextMenuTargets::Queue(slots.clone()),
        None,
    ))));
    let index = match &app.gui_overlay {
        Some(GuiOverlay::RowMenu(menu)) => menu
            .entries
            .iter()
            .position(|entry| matches!(entry.action, Some(ContextAction::RemoveSelection(_))))
            .expect("the bulk remove entry listed"),
        _ => panic!("the selection menu did not open"),
    };

    app.select_gui_row_menu_entry(index);

    let removed: Vec<u64> = cmd_rx
        .try_recv()
        .ok()
        .and_then(|cmd| match cmd {
            CtrlCmd::UnifiedQueueRemoveSlots { slot_ids, .. } => Some(slot_ids),
            _ => None,
        })
        .expect("the owner received one RemoveSlots edit");
    assert_eq!(
        removed,
        slots.iter().map(|slot| slot.raw()).collect::<Vec<_>>()
    );
    assert!(app.gui_overlay.is_none());
}

// An unannounced local-Owner loss raises the recovery dialog
// (`daemon-disconnect-handling`: "An unannounced connection loss raises a
// recovery dialog"). The App's own raise presents it as the GUI's blocking
// overlay, the frame carries the same diagnostics and the three recovery
// choices the TUI's hint line offers, and the dialog blocks every other
// input — Esc and an outside press included — until a choice runs.
#[test]
fn unannounced_loss_raises_the_gui_daemon_lost_dialog() {
    let mut app = make_app_stub();
    app.enable_gui_client();
    app.playback_queue_mut().adopt_items(make_items(2), 0);

    app.raise_daemon_lost_modal();

    let playback = app.queue_playback_projection();
    let Some(GuiOverlayProjection::DaemonLost {
        last_playing_title,
        daemon_log_path,
        restart_error,
        buttons,
    }) = app.gui_overlay_projection(&playback)
    else {
        panic!("the unannounced loss raised the dialog");
    };
    assert_eq!(last_playing_title.as_deref(), Some("Item 0"));
    assert!(daemon_log_path.ends_with("local-daemon.log"));
    assert!(restart_error.is_none(), "no restart has failed yet");
    let choices: Vec<(&str, &str)> = buttons
        .iter()
        .map(|button| (button.label.as_str(), button.keys.as_str()))
        .collect();
    assert_eq!(
        choices,
        [
            ("Restart and resume", "R"),
            ("Restart, don't resume", "S"),
            ("Quit", "Q"),
        ]
    );

    // No chord but the three choices does anything: the dialog's own key
    // rules bind no default answer, and Esc must not dismiss it back to a
    // dead Owner.
    for code in [Key::Esc, Key::Enter, Key::Char('x')] {
        app.route_gui_input(GuiInput::Key(KeyEvent::new(code, KeyModifiers::NONE)), None);
        assert!(app.gui_overlay_is_open(), "{code:?} left the dialog up");
    }
    // An outside press leaves it up too (the confirm modal's opposite: the
    // recovery choice is the only way out).
    app.dismiss_open_gui_overlay();
    assert!(app.gui_overlay_is_open());
}
