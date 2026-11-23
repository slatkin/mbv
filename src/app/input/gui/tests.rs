use std::sync::mpsc::Receiver;

use mbv_ctrl::{CtrlCmd, WireCommand};
use mbv_ui_model::context_menu::ContextAction;
use mbv_ui_model::overlay::OverlayRequest;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, Msg, PlaybackRequest, ShellRequest, TrackChipKind};

use crate::app::App;
use crate::app::gui_client_state::{GuiOverlay, GuiOverlayProjection};
use crate::app::input::gui::GuiInput;
use crate::app::tests::{
    QueueViewTestExt, gui_client_capabilities, make_app_stub, make_items,
    make_remote_app_stub_with_cmd_rx,
};
use crate::gui::GuiCore;

/// Stands in for the GUI core thread's drain of the two modal requests
/// (`crates/mbv-gui` `apply_pending_overlay_request`): the App only raises
/// the request, and the GUI drain presents it.
fn present_pending_modal(core: &mut GuiCore) {
    match core.app.take_pending_overlay() {
        Some(OverlayRequest::Confirm(modal)) => core.present_gui_confirm(modal),
        Some(OverlayRequest::DaemonLost(modal)) => core.present_gui_daemon_lost(modal),
        other => panic!("expected a modal request, got {other:?}"),
    }
}

/// One routed key, delivered through the GUI entry.
fn gui_key(core: &mut GuiCore, input: GuiInput) -> bool {
    core.route_gui_input(input, None)
}

/// One surface message, delivered through the GUI entry.
fn gui_msg(core: &mut GuiCore, msg: Msg) {
    core.handle_gui_surface_msg(msg);
}

/// A stub `App` in its GUI core, with a command receiver for the Owner.
fn core_with_cmd_rx() -> (GuiCore, Receiver<CtrlCmd>) {
    let (app, cmd_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    (GuiCore::new(app), cmd_rx)
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
    let mut core = GuiCore::new(make_app_stub());
    core.app.playback_queue_mut().adopt_items(make_items(2), 0);
    core.app.set_panel_focus(PanelFocus::Library);
    let row = core
        .app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::QueueRowClick { slot_id: Some(row) })),
    );

    assert_eq!(core.app.effective_panel_focus(), PanelFocus::Queue);
}

// U17 review finding 2: a GUI double click on a queue row plays that row
// through the TUI double click's path, so the owner receives the play of the
// row's slot.
#[test]
fn queue_row_activate_plays_the_clicked_row_review_u17_2() {
    let (mut core, cmd_rx) = core_with_cmd_rx();
    // The direct-remote stub displays its Peer queue, so the rows go there.
    core.app.playback_queue_mut().adopt_items(make_items(3), 0);
    let row = core
        .app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::QueueRowActivate {
            slot_id: Some(row),
        })),
    );

    assert_eq!(played_slot_ids(&cmd_rx), vec![row.raw()]);
}

// The GUI-only key context (add-tauri-gui-client row 6.2): the canvas's `z`
// collapses the panel, and the same chord stays `cycle_subtitle`'s in the TUI
// — the routing matrix pins the TUI side, this pins the GUI side.
#[test]
fn gui_context_z_toggles_the_panel_collapse() {
    let mut core = GuiCore::new(make_app_stub());
    core.app.set_client_capabilities(gui_client_capabilities());

    assert!(!gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('z'), KeyModifiers::NONE)),
    ));
    assert!(core.app.gui.panel_toggles.collapsed);

    assert!(!gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('z'), KeyModifiers::NONE)),
    ));
    assert!(!core.app.gui.panel_toggles.collapsed);
}

// The GUI has no Panel focus (ADR 0040, design D3 of
// `gui-pointer-target-interaction`): Ctrl+Right, the TUI's chord that hands
// Panel focus to the Library, changes nothing here. A key names its target
// instead, and the core sets the stack's Panel focus from it per key.
#[test]
fn the_gui_panel_focus_chord_changes_no_panel_focus() {
    let mut core = GuiCore::new(make_app_stub());
    core.app.set_client_capabilities(gui_client_capabilities());
    core.app.set_panel_focus(PanelFocus::Queue);

    assert!(!gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Right, KeyModifiers::CONTROL)),
    ));

    assert_eq!(core.app.effective_panel_focus(), PanelFocus::Queue);
}

// The GUI-only key context's `q` folds the queue; the TUI's `quit` on the
// same chord is untouched there (the routing matrix pins it).
#[test]
fn gui_context_q_toggles_the_queue_fold() {
    let mut core = GuiCore::new(make_app_stub());
    core.app.set_client_capabilities(gui_client_capabilities());

    assert!(!gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('q'), KeyModifiers::NONE)),
    ));
    assert!(core.app.gui.panel_toggles.queue_folded);
}

// The GUI-only key context's `c` cycles the subtitle track, the chord the
// TUI binds to the clear-queue prompt. With the stub's idle player the
// dispatch lands in the subtitle-mode cycle, so the config value is the
// observable.
#[test]
fn gui_context_c_cycles_the_subtitle_track() {
    let mut core = GuiCore::new(make_app_stub());
    core.app.set_client_capabilities(gui_client_capabilities());
    // The stub player starts active; the subtitle cycle's active path needs
    // tracks the stub does not carry, so the test drives the idle path the
    // mode cycle lives on.
    core.app
        .player
        .update_status(|status| status.active = false);
    let before = core
        .app
        .config
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .subtitle_mode
        .clone();

    assert!(!gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('c'), KeyModifiers::NONE)),
    ));

    let after = core
        .app
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
    let mut core = GuiCore::new(make_app_stub());
    core.app.set_client_capabilities(gui_client_capabilities());
    let key = |c: char| GuiInput::Key(KeyEvent::new(Key::Char(c), KeyModifiers::NONE));

    gui_key(&mut core, key('/'));
    assert!(core.app.queue_filter_active);

    gui_key(&mut core, key('a'));
    gui_key(&mut core, key('b'));
    assert_eq!(core.app.queue_filter, "ab");

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Backspace, KeyModifiers::NONE)),
    );
    assert_eq!(core.app.queue_filter, "a");

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
    );
    assert!(!core.app.queue_filter_active);
    assert_eq!(core.app.queue_filter, "");
}

// The GUI's row menu opens from the row's context-menu request — the
// keyboard's `.` and the right-click resolve through the same request — with
// the canvas's entries and their key hints.
#[test]
fn row_context_menu_request_opens_the_row_menu_with_hints() {
    let mut core = GuiCore::new(make_app_stub());
    core.app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = core
        .app
        .displayed_queue()
        .slot_id_at(0)
        .expect("row 0 has a slot");

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        ))),
    );

    let Some(GuiOverlay::RowMenu(menu)) = &core.app.gui.overlay else {
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
// other entry, so the window paints the canvas's tone from the flag and
// never from a label or a position.
#[test]
fn the_row_menu_projects_only_the_remove_entry_as_destructive() {
    let mut core = GuiCore::new(make_app_stub());
    core.app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = core
        .app
        .displayed_queue()
        .slot_id_at(0)
        .expect("row 0 has a slot");
    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        ))),
    );

    let playback = core.app.queue_playback_projection();
    let Some(GuiOverlayProjection::RowMenu { entries, .. }) =
        core.gui_overlay_projection(&playback)
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
    let (mut core, cmd_rx) = core_with_cmd_rx();
    // The direct-remote stub displays its Peer queue, so the rows go there.
    core.app.playback_queue_mut().adopt_items(make_items(3), 0);
    let row = core
        .app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");
    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        ))),
    );

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
    );

    assert_eq!(played_slot_ids(&cmd_rx), vec![row.raw()]);
    assert!(core.app.gui.overlay.is_none());
}

// The canvas's two-stroke `dd` hint on Remove: the first `d` arms, the
// second removes the retained row, so the owner receives that slot's
// remove. The menu opens on a row below the playhead, so the remove is the
// plain slot remove.
#[test]
fn row_menu_two_stroke_dd_removes_the_row() {
    let (mut core, cmd_rx) = core_with_cmd_rx();
    core.app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = core
        .app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");
    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        ))),
    );

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('d'), KeyModifiers::NONE)),
    );
    // The first stroke arms only.
    assert!(core.app.gui.overlay.is_some());
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('d'), KeyModifiers::NONE)),
    );

    let removed: Vec<u64> = cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CtrlCmd::UnifiedQueueRemoveSlot { slot_id, .. } => Some(slot_id),
            _ => None,
        })
        .collect();
    assert_eq!(removed, vec![row.raw()]);
    assert!(core.app.gui.overlay.is_none());
}

// The open overlay owns the keyboard while it is up: a chord its rules do
// not bind is dropped — the panel beneath it sees nothing — and Esc
// dismisses.
#[test]
fn open_overlay_owns_the_keyboard() {
    let mut core = GuiCore::new(make_app_stub());
    core.app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = core
        .app
        .displayed_queue()
        .slot_id_at(0)
        .expect("row 0 has a slot");
    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        ))),
    );

    // The GUI-only collapse chord is a menu-owned chord while the menu is
    // open: it drops, and the panel does not collapse.
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('z'), KeyModifiers::NONE)),
    );
    assert!(!core.app.gui.panel_toggles.collapsed);
    assert!(core.app.gui.overlay.is_some());

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
    );
    assert!(core.app.gui.overlay.is_none());
}

// The track popover's keyboard choice sends the Owner the chosen track: Down
// past the Off entry, Enter on the track, and the popover closes.
#[test]
fn track_popover_choose_sends_the_track_switch() {
    let _state_dir = crate::config::TestStateDirGuard::new();
    let (mut core, cmd_rx) = core_with_cmd_rx();
    core.app.set_client_capabilities(gui_client_capabilities());
    core.app.player.update_status(|status| {
        status.active = true;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });

    // The GUI Client's playback entry runs the shared effect and then its own
    // half of the chip press (design D4a (a)).
    gui_msg(
        &mut core,
        Msg::Playback(PlaybackRequest::OpenTrackPopover(TrackChipKind::Subtitles)),
    );
    let Some(GuiOverlay::TrackPopover(popover)) = &core.app.gui.overlay else {
        panic!("the popover opened over the chip's choices");
    };
    assert_eq!(popover.cursor, 0, "the cursor starts on the chosen Off");

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Down, KeyModifiers::NONE)),
    );
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
    );

    let switched: Vec<i64> = cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CtrlCmd::PlayerCmd(WireCommand::SetSub(id)) => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(switched, vec![2]);
    assert!(core.app.gui.overlay.is_none());
}

// Unit 6.2 review finding (P2): a track popover left open when the local
// player goes inactive has no chip source, so the frame projects no overlay
// for it. The held state drops with its source before the snapshot reads
// the overlay focus — otherwise the global gates stay closed and the
// overlay's key rules swallow input the window does not paint.
#[test]
fn track_popover_drops_when_its_chips_disappear_review_u62_p2() {
    let mut core = GuiCore::new(make_app_stub());
    core.app.set_client_capabilities(gui_client_capabilities());
    core.app.player.update_status(|status| {
        status.active = true;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });
    gui_msg(
        &mut core,
        Msg::Playback(PlaybackRequest::OpenTrackPopover(TrackChipKind::Subtitles)),
    );
    assert!(core.app.gui.overlay.is_some());

    // Playback ends while the popover is up (the queue exhausts, or the
    // Owner stops from another client): the chips' source is gone.
    core.app
        .player
        .update_status(|status| status.active = false);

    // The next chord reconciles the orphaned popover before the snapshot,
    // so it routes to its binding instead of being consumed by the open
    // overlay's key rules.
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('z'), KeyModifiers::NONE)),
    );

    assert!(core.app.gui.overlay.is_none());
    assert!(
        core.app.gui.panel_toggles.collapsed,
        "the chord routed to its binding"
    );
}

// The canvas's back and forward controls emit the deltas the
// `seek_back_10` and `seek_forward_30` chords carry (add-tauri-gui-client
// row 6.4, the "back 10 s, forward 30 s" spec): the request's dispatch
// and the chords' dispatch send the same command. The TUI's `<`/`>` keep
// their ±5 s.
#[test]
fn seek_relative_request_matches_the_seek_chords() {
    let (mut core, cmd_rx) = core_with_cmd_rx();
    core.app.set_client_capabilities(gui_client_capabilities());

    core.app
        .handle_playback_request(&PlaybackRequest::SeekRelative(-10.0));
    core.app
        .handle_playback_request(&PlaybackRequest::SeekRelative(30.0));
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char('['), KeyModifiers::NONE)),
    );
    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Char(']'), KeyModifiers::NONE)),
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
    let mut core = GuiCore::new(make_app_stub());
    core.app.set_client_capabilities(gui_client_capabilities());
    core.app.player.update_status(|status| {
        status.active = true;
        status.audio_tracks = vec![(1, "Stereo".to_string())];
        status.audio_id = 1;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });
    let key = |c: char| GuiInput::Key(KeyEvent::new(Key::Char(c), KeyModifiers::NONE));
    let open_chip = |app: &App| match &app.gui.overlay {
        Some(GuiOverlay::TrackPopover(popover)) => Some(popover.chip),
        _ => None,
    };

    gui_key(&mut core, key('a'));
    assert_eq!(open_chip(&core.app), Some(TrackChipKind::Audio));

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
    );
    assert!(core.app.gui.overlay.is_none());

    gui_key(&mut core, key('s'));
    assert_eq!(open_chip(&core.app), Some(TrackChipKind::Subtitles));
}

// The GUI's QueueIntent::Clear runs the TUI's own App action: the `c`
// chord's request, which raises the shared clear-queue confirmation the
// GUI now presents as its own overlay (add-gui-library-panel queue fixes —
// the prompt used to be a documented no-op, so the GUI had no clear at
// all). Enter on the overlay answers the confirm, and the owner receives
// the answered clear op the TUI's confirm sends.
#[test]
fn queue_intent_clear_opens_the_confirm_overlay_and_enter_clears_the_queue() {
    let (mut core, cmd_rx) = core_with_cmd_rx();
    core.app.set_client_capabilities(gui_client_capabilities());
    core.app.playback_queue_mut().adopt_items(make_items(2), 0);

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::QueueIntent(
            mbv_ui_msg::QueueIntent::Clear,
        ))),
    );
    present_pending_modal(&mut core);

    let playback = core.app.queue_playback_projection();
    let Some(GuiOverlayProjection::Confirm { message, buttons }) =
        core.gui_overlay_projection(&playback)
    else {
        panic!("the clear confirm presented");
    };
    assert_eq!(message, "Clear the queue?");
    assert_eq!(buttons.len(), 2);

    gui_key(
        &mut core,
        GuiInput::Key(KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
    );

    assert!(core.app.gui.overlay.is_none());
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
    let mut core = GuiCore::new(make_app_stub());
    core.app.set_client_capabilities(gui_client_capabilities());
    core.app.playback_queue_mut().adopt_items(make_items(2), 0);
    let row = core
        .app
        .displayed_queue()
        .slot_id_at(0)
        .expect("row 0 has a slot");

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Queue(vec![row]),
        ))),
    );
    let index = match &core.app.gui.overlay {
        Some(GuiOverlay::RowMenu(menu)) => menu
            .entries
            .iter()
            .position(|entry| matches!(entry.action, Some(ContextAction::ClearQueue)))
            .expect("the clear entry listed"),
        _ => panic!("the row menu did not open"),
    };

    core.select_gui_row_menu_entry(index);
    present_pending_modal(&mut core);

    match &core.app.gui.overlay {
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
    let (mut core, cmd_rx) = core_with_cmd_rx();
    core.app.set_client_capabilities(gui_client_capabilities());
    core.app.playback_queue_mut().adopt_items(make_items(3), 0);
    // The stub's player reports slot 0 as the now-playing row, whose removal
    // owns its own confirmation; the selection under test is the two rows
    // behind it, so the edit goes out without a prompt.
    let slots: Vec<mbv_queue::QueueSlotId> = (1..3)
        .filter_map(|index| core.app.displayed_queue().slot_id_at(index))
        .collect();

    gui_msg(
        &mut core,
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Queue(slots.clone()),
        ))),
    );
    let index = match &core.app.gui.overlay {
        Some(GuiOverlay::RowMenu(menu)) => menu
            .entries
            .iter()
            .position(|entry| matches!(entry.action, Some(ContextAction::RemoveSelection(_))))
            .expect("the bulk remove entry listed"),
        _ => panic!("the selection menu did not open"),
    };

    core.select_gui_row_menu_entry(index);

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
    assert!(core.app.gui.overlay.is_none());
}

// An unannounced local-Owner loss raises the recovery dialog
// (`daemon-disconnect-handling`: "An unannounced connection loss raises a
// recovery dialog"). The App's own raise presents it as the GUI's blocking
// overlay, the frame carries the same diagnostics and the three recovery
// choices the TUI's hint line offers, and the dialog blocks every other
// input — Esc and an outside press included — until a choice runs.
#[test]
fn unannounced_loss_raises_the_gui_daemon_lost_dialog() {
    let mut core = GuiCore::new(make_app_stub());
    core.app.set_client_capabilities(gui_client_capabilities());
    core.app.playback_queue_mut().adopt_items(make_items(2), 0);

    core.app.raise_daemon_lost_modal();
    present_pending_modal(&mut core);

    let playback = core.app.queue_playback_projection();
    let Some(GuiOverlayProjection::DaemonLost {
        last_playing_title,
        daemon_log_path,
        restart_error,
        buttons,
    }) = core.gui_overlay_projection(&playback)
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
        gui_key(
            &mut core,
            GuiInput::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        );
        assert!(
            core.app.gui.overlay_is_open(),
            "{code:?} left the dialog up"
        );
    }
    // An outside press leaves it up too (the confirm modal's opposite: the
    // recovery choice is the only way out).
    core.dismiss_open_gui_overlay();
    assert!(core.app.gui.overlay_is_open());
}

mod destination_tabs;
mod player_menu;
mod queue_chooser;
mod target_chooser;
