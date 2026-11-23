//! The Queue chooser's GUI routing (gui-named-queues design D8): the `t`
//! chord opens and closes it with the cursor on the shown entry, Enter shows
//! the entry under the cursor, and `r` renames the local queue under it.

use std::sync::mpsc::Receiver;

use mbv_ctrl::CtrlCmd;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, Msg, PlaybackRequest};

use crate::app::App;
use crate::app::gui_client_state::{GuiOverlay, GuiOverlayProjection};
use crate::app::input::gui::GuiInput;
use crate::app::tests::{
    emby_unified_state, gui_client_capabilities, make_items, make_local_daemon_app_stub_with_cmd_rx,
};
use mbv_ctrl::player::PlayerEvent;

/// The stub Owner's current queue.
fn current() -> mbv_queue::NamedQueueId {
    mbv_queue::NamedQueueId::from_raw(1)
}

/// The stub Owner's resting queue.
fn resting() -> mbv_queue::NamedQueueId {
    mbv_queue::NamedQueueId::from_raw(2)
}

/// A GUI Client attached to a local Owner that holds `Default` (current) and
/// `Kids` (resting), with the Queue as the pointer target and the Owner's
/// traffic drained.
fn app_with_queues() -> (App, Receiver<CtrlCmd>) {
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(3));
    while cmd_rx.try_recv().is_ok() {}
    app.set_client_capabilities(gui_client_capabilities());
    app.set_panel_focus(PanelFocus::Queue);
    app.adopt_named_queue_list(
        vec![
            mbv_ctrl::NamedQueueEntry {
                id: current(),
                name: "Default".to_string(),
                len: 3,
            },
            mbv_ctrl::NamedQueueEntry {
                id: resting(),
                name: "Kids".to_string(),
                len: 1,
            },
        ],
        current(),
    );
    (app, cmd_rx)
}

/// The `t` chord, the next key in a test's sequence.
fn press(app: &mut App, key: KeyEvent) {
    app.route_gui_input(GuiInput::Key(key), None);
}

/// The unmodified letter chord `c` presses.
fn letter(c: char) -> KeyEvent {
    KeyEvent::new(Key::Char(c), KeyModifiers::NONE)
}

/// Pressing `t` over the Queue list box opens the chooser with its cursor on
/// the shown entry, and `t` again closes it (spec `gui-queue-chooser` "Open
/// the chooser by key").
#[test]
fn the_chooser_opens_by_key_with_its_cursor_on_the_shown_entry() {
    let (mut app, _cmd_rx) = app_with_queues();
    app.show_named_queue(resting());

    press(&mut app, letter('t'));

    let Some(GuiOverlay::QueueChooser(chooser)) = &app.gui_overlay else {
        panic!("the `t` chord opened the Queue chooser");
    };
    assert_eq!(
        chooser.cursor, 1,
        "the cursor rests on the shown entry, the second local queue"
    );

    press(&mut app, letter('t'));
    assert!(app.gui_overlay.is_none(), "`t` closes the open chooser");
}

/// The chooser lists the local queues in queue order and then the peer, marks
/// the shown and current entries, and Enter on the peer entry shows it
/// (spec `gui-queue-chooser` "The chooser lists local queues and connected
/// peers").
#[test]
fn the_chooser_projects_the_local_queues_then_the_peer() {
    let (mut app, _cmd_rx) = app_with_queues();
    app.show_named_queue(resting());

    let entries = app.queue_chooser_entries();

    let labels: Vec<&str> = entries.iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(labels, ["Default", "Kids"]);
    let marks: Vec<(bool, bool)> = entries
        .iter()
        .map(|entry| (entry.shown, entry.current))
        .collect();
    assert_eq!(
        marks,
        [(false, true), (true, false)],
        "`Default` is current and `Kids` is shown"
    );
}

/// The open chooser takes its keys: Down moves the cursor, Enter shows the
/// entry under it, and showing a resting queue asks the Owner for its
/// snapshots (spec `gui-queue-chooser` "The open chooser takes its keys").
#[test]
fn the_open_chooser_shows_the_entry_under_its_cursor() {
    let (mut app, cmd_rx) = app_with_queues();
    app.show_local_current_entry();

    press(&mut app, letter('t'));
    press(&mut app, KeyEvent::new(Key::Down, KeyModifiers::NONE));
    press(&mut app, KeyEvent::new(Key::Enter, KeyModifiers::NONE));

    assert!(app.gui_overlay.is_none(), "Enter closes the chooser");
    assert!(
        matches!(
            cmd_rx.try_recv(),
            Ok(CtrlCmd::NamedQueueShow { queue }) if queue == resting()
        ),
        "the shown resting queue is the one the Owner subscribes"
    );
}

/// `r` opens the name of the local queue under the cursor for edit with the
/// whole name selected, so the typed text replaces it, and Enter sends one
/// `NamedQueueRename` for that queue (spec `gui-queue-chooser` "Rename by
/// key"; user request 2026-11-17).
#[test]
fn a_queue_renames_by_key_from_the_chooser() {
    let (mut app, cmd_rx) = app_with_queues();

    press(&mut app, letter('t'));
    press(&mut app, KeyEvent::new(Key::Down, KeyModifiers::NONE));
    press(&mut app, letter('r'));
    press(&mut app, letter('Z'));
    press(&mut app, letter('o'));
    press(&mut app, KeyEvent::new(Key::Enter, KeyModifiers::NONE));

    assert!(
        matches!(
            cmd_rx.try_recv(),
            Ok(CtrlCmd::NamedQueueRename { queue, name, .. })
                if queue == resting() && name == "Zo"
        ),
        "the rename names the queue under the cursor and the typed text alone"
    );
    assert!(app.gui_overlay.is_none(), "the confirmed rename closes it");
}

/// A name open for edit is selected, so Enter accepts it unchanged and
/// Backspace clears it: a user never has to select the default name first.
#[test]
fn an_open_name_is_selected_so_enter_keeps_it_and_backspace_clears_it() {
    let (mut app, cmd_rx) = app_with_queues();
    press(&mut app, letter('t'));
    press(&mut app, KeyEvent::new(Key::Down, KeyModifiers::NONE));
    press(&mut app, letter('r'));
    press(&mut app, KeyEvent::new(Key::Enter, KeyModifiers::NONE));
    assert!(
        matches!(
            cmd_rx.try_recv(),
            Ok(CtrlCmd::NamedQueueRename { name, .. }) if name == "Kids"
        ),
        "Enter on the untouched draft keeps the name"
    );

    press(&mut app, letter('t'));
    press(&mut app, KeyEvent::new(Key::Down, KeyModifiers::NONE));
    press(&mut app, letter('r'));
    press(&mut app, KeyEvent::new(Key::Backspace, KeyModifiers::NONE));
    let Some(GuiOverlay::QueueChooser(chooser)) = &app.gui_overlay else {
        panic!("the chooser is open");
    };
    let draft = chooser.renaming.as_ref().expect("the name is open");
    assert_eq!(draft.text, "", "Backspace clears the selected name");
    assert!(!draft.selected, "the selection is spent");
}

/// The peer entry cannot be renamed: `r` on it opens no draft, so Enter shows
/// the entry instead of naming it (spec `gui-queue-chooser`: "A peer entry
/// SHALL NOT be renamed").
#[test]
fn a_peer_entry_cannot_be_renamed() {
    let (mut app, cmd_rx) = app_with_queues();
    let (peer, peer_rx) = mbv_remote_player::RemotePlayer::stub(make_items(2), 0);
    let sess = mbv_emby::test_support::make_session("peer-host", "mbv");
    app.switch_to_direct_remote(
        &sess,
        peer,
        peer_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().expect("a test address")),
    );
    while cmd_rx.try_recv().is_ok() {}

    press(&mut app, letter('t'));
    // The peer entry is last and the connect showed it, so the cursor opens
    // on it: `r` is the chord under test.
    press(&mut app, letter('r'));

    let Some(GuiOverlay::QueueChooser(chooser)) = &app.gui_overlay else {
        panic!("the chooser is open");
    };
    assert!(chooser.renaming.is_none(), "a peer entry opens no draft");

    press(&mut app, KeyEvent::new(Key::Enter, KeyModifiers::NONE));
    assert!(
        app.queue_chooser_entries()
            .last()
            .is_some_and(|entry| entry.shown && entry.queue.is_none()),
        "Enter shows the peer entry"
    );
}

/// The open chooser projects its entries and rename draft for the window, so
/// the dropdown paints the marks and the inline name the core holds.
#[test]
fn the_chooser_projects_its_entries_and_rename_draft() {
    let (mut app, _cmd_rx) = app_with_queues();
    press(&mut app, letter('t'));
    press(&mut app, letter('r'));

    let playback = app.queue_playback_projection();
    let Some(GuiOverlayProjection::QueueChooser {
        cursor,
        entries,
        renaming,
    }) = app.gui_overlay_projection(&playback)
    else {
        panic!("the chooser projects");
    };
    assert_eq!(cursor, 0);
    assert_eq!(entries.len(), 2);
    let renames = renaming.expect("the draft projects");
    assert_eq!(renames.queue, current());
    assert_eq!(renames.text, "Default");
    assert!(
        renames.selected,
        "the draft opens with its whole name selected"
    );
}

/// The chooser counts each entry's queue (spec `gui-queue-chooser` "Each
/// entry shows its slot count", design D8 of `gui-queue-panel-redesign`): the
/// current queue's live view, a resting queue's published length, and the
/// peer's own view. The header's switcher carries the shown queue's count.
/// (`Default` holds 3 slots, `Kids` publishes 1, and the peer holds 2.)
#[test]
fn the_chooser_counts_the_current_resting_and_peer_queues() {
    let (mut app, _cmd_rx) = app_with_queues();
    let (peer, peer_rx) = mbv_remote_player::RemotePlayer::stub(make_items(2), 0);
    let sess = mbv_emby::test_support::make_session("peer-host", "mbv");
    app.switch_to_direct_remote(
        &sess,
        peer,
        peer_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().expect("a test address")),
    );

    let counts: Vec<usize> = app
        .queue_chooser_entries()
        .iter()
        .map(|entry| entry.len)
        .collect();

    assert_eq!(counts, [3, 1, 2], "the current, the resting, and the peer");
    assert_eq!(
        app.gui_queue_header_projection().shown_len,
        2,
        "the switcher counts the shown queue, which the connect made the peer"
    );
}

/// The current queue's count follows its edits (spec `gui-queue-chooser` "The
/// current queue's count follows edits", design D8 of
/// `gui-queue-panel-redesign`): it reads the current queue's own snapshot, not
/// the queue list the Owner publishes on create, rename, close, and swap.
#[test]
fn the_current_queues_count_follows_a_removal() {
    let (mut app, _cmd_rx) = app_with_queues();
    app.show_local_current_entry();

    app.handle_player_event(PlayerEvent::UnifiedQueueUpdated(Box::new(
        emby_unified_state(&make_items(2), 0),
    )));

    assert_eq!(
        app.queue_chooser_entries().first().map(|entry| entry.len),
        Some(2),
        "the current queue's count follows the removal"
    );
    assert_eq!(
        app.gui_queue_header_projection().shown_len,
        2,
        "the switcher follows the same removal"
    );
}

/// The three Queue chords act only while the pointer target is the Queue list
/// box (spec `gui-queue-chooser` "The chooser and its controls are reachable
/// from the keyboard"): with the Library column under the pointer, `t` opens
/// no chooser and `Ctrl+T` and `Ctrl+W` reach no control.
#[test]
fn the_queue_chords_need_the_queue_pointer_target() {
    let (mut app, _cmd_rx) = app_with_queues();
    app.set_panel_focus(PanelFocus::Library);

    press(&mut app, letter('t'));
    press(
        &mut app,
        KeyEvent::new(Key::Char('t'), KeyModifiers::CONTROL),
    );
    press(
        &mut app,
        KeyEvent::new(Key::Char('w'), KeyModifiers::CONTROL),
    );

    assert!(
        app.gui_overlay.is_none(),
        "no Queue chord opened the chooser over the Library column"
    );
}

/// The queue header's Queue chooser control presses through the GUI Client's
/// playback entry, not the router: the press emits
/// `PlaybackRequest::QueueChooserToggle`, whose core arm is a documented no-op
/// (design D4a (a)), so the GUI entry must run the toggle (D8, "the header's
/// chooser press"). Without it the header button did nothing while the `t`
/// chord still worked (row 5.1 review P1a).
#[test]
fn the_header_chooser_press_opens_the_chooser_review_p1a() {
    let (mut app, _cmd_rx) = app_with_queues();

    app.handle_gui_surface_msg(Msg::Playback(PlaybackRequest::QueueChooserToggle));

    assert!(
        matches!(app.gui_overlay, Some(GuiOverlay::QueueChooser(_))),
        "the header press opened the Queue chooser"
    );
}
