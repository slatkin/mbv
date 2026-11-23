//! The Client core's GUI queue controls (gui-named-queues design D8): the
//! new-queue control and the close control, each one path the header's press and
//! its GUI-only chord share, and the Queue chooser's entry press (task 8.3),
//! which runs the show its Enter runs. Specs: `gui-queue-chooser` ("The
//! new-queue control makes and shows a queue", "The close control closes the
//! shown entry", "The chooser lists local queues and connected peers").

use super::named_queues::peer_link::gui_app_with_peer;
use super::named_queues::{
    current, gui_app_with_named_queues, inject_local_events, named_queue_list, op_answer, resting,
    resting_snapshot,
};
use super::*;
use crate::app::gui_client_state::GuiOverlay;
use crate::app::state::playback::ShownEntry;
use mbv_ctrl::{CtrlCmd, NamedQueueEntry, UnifiedQueueStateData};
use mbv_queue::NamedQueueId;
use mbv_ui_model::overlay::OverlayRequest;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, Msg, PlaybackRequest};
use rstest::rstest;

/// The queue the Owner mints for the first new queue beside `Default`.
fn new_queue() -> NamedQueueId {
    NamedQueueId::from_raw(3)
}

/// The queue list the Owner publishes after it makes [`new_queue`].
fn list_with_new_queue() -> Vec<NamedQueueEntry> {
    let mut entries = named_queue_list();
    entries.push(NamedQueueEntry {
        id: new_queue(),
        name: "Queue 3".to_string(),
        len: 0,
    });
    entries
}

/// The new queue's own snapshot, as the answer to the create carries it.
fn new_queue_snapshot() -> UnifiedQueueStateData {
    UnifiedQueueStateData {
        queue: Some(new_queue()),
        ..resting_snapshot(&[])
    }
}

/// The new queue's own snapshot holding `items`, as the answer to an edit of it
/// carries it.
fn new_queue_with(items: &[EmbyItem]) -> UnifiedQueueStateData {
    UnifiedQueueStateData {
        queue: Some(new_queue()),
        ..resting_snapshot(items)
    }
}

/// One key of an answer to the shared confirmation modal.
fn answer_key(code: Key) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The close request the frame's control sends: the shown entry's close.
fn request_close(app: &mut App) {
    app.dispatch(&AppCommand::Panel(PanelIntent::RequestCloseQueue));
}

/// The `op` of the next `NamedQueueCreate` in `rx`, or `None` when the Owner is
/// sent none.
fn sent_create(rx: &std::sync::mpsc::Receiver<CtrlCmd>) -> Option<mbv_ctrl::QueueOpId> {
    rx.try_iter().find_map(|cmd| match cmd {
        CtrlCmd::NamedQueueCreate { op } => Some(op),
        _ => None,
    })
}

/// Whether `rx` holds a `NamedQueueShow` for `queue`, among the commands the
/// Client sent.
fn sent_show(rx: &std::sync::mpsc::Receiver<CtrlCmd>, queue: NamedQueueId) -> bool {
    rx.try_iter()
        .any(|cmd| matches!(cmd, CtrlCmd::NamedQueueShow { queue: shown } if shown == queue))
}

/// The `queue` of one `NamedQueueClose` in `rx`, or `None` when the Owner is
/// sent none.
fn sent_close(rx: &std::sync::mpsc::Receiver<CtrlCmd>) -> Option<NamedQueueId> {
    rx.try_iter().find_map(|cmd| match cmd {
        CtrlCmd::NamedQueueClose { queue, .. } => Some(queue),
        _ => None,
    })
}

/// The `op` of one `NamedQueueClose` in `rx`, or `None` when the Owner is sent
/// none.
fn sent_close_op(rx: &std::sync::mpsc::Receiver<CtrlCmd>) -> Option<mbv_ctrl::QueueOpId> {
    rx.try_iter().find_map(|cmd| match cmd {
        CtrlCmd::NamedQueueClose { op, .. } => Some(op),
        _ => None,
    })
}

/// A refused close tells the user why instead of no-oping: the Owner refuses
/// its last queue and a stale list may refuse any close (design D8, spec
/// `named-queues` "A Client closes queues").
#[test]
fn a_rejected_close_tells_the_user_why() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_local_current_entry();
    app.player.update_status(|status| status.active = false);
    request_close(&mut app);
    let op = sent_close_op(&cmd_rx).expect("the close reaches the Owner");
    app.toasts.clear();

    app.handle_player_event(PlayerEvent::QueueOpResult {
        op,
        outcome: mbv_ctrl::QueueOpOutcome::Rejected("The last queue cannot be closed".into()),
    });

    assert_eq!(
        app.newest_toast_text(),
        "The last queue cannot be closed",
        "the refusal says why the queue stayed"
    );
    assert_eq!(
        app.newest_toast_severity(),
        crate::app::dispatch::notify::ToastSeverity::Error,
        "a refusal is an error, not a neutral status"
    );
    assert!(
        app.pending_named_queue_ops.is_empty(),
        "the answered close ends its request"
    );
}

/// The new-queue control asks the local Owner for a queue, and the answer that
/// names it shows it with its name open for edit (spec `gui-queue-chooser`
/// "Make a queue").
#[test]
fn the_new_queue_control_shows_the_new_queue_with_its_name_open() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();

    app.dispatch(&AppCommand::Panel(PanelIntent::RequestNewQueue));
    app.dispatch(&AppCommand::Panel(PanelIntent::RequestNewQueue));

    let Ok(CtrlCmd::NamedQueueCreate { op }) = cmd_rx.try_recv() else {
        panic!("the control asks the local Owner for a new queue");
    };
    assert!(
        cmd_rx.try_recv().is_err(),
        "a press while the Owner has not answered makes no second queue"
    );
    // The Owner publishes the queue list and then answers the request with the
    // new queue's own snapshot, under the request's `op`.
    app.adopt_named_queue_list(list_with_new_queue(), current());
    app.handle_player_event(op_answer(op.0, new_queue_snapshot()));

    assert_eq!(
        app.shown_entry,
        Some(ShownEntry::Local(new_queue())),
        "the Queue panel shows the queue the Owner reported"
    );
    assert_eq!(
        app.displayed_queue().slots().len(),
        0,
        "the new queue is empty"
    );
    let Some(GuiOverlay::QueueChooser(chooser)) = &app.gui.overlay else {
        panic!("the new queue's name is open for edit in the chooser");
    };
    assert_eq!(chooser.cursor, 2, "the cursor rests on the new queue");
    let draft = chooser.renaming.as_ref().expect("the name is open");
    assert_eq!(draft.queue, new_queue());
    assert_eq!(draft.text, "Queue 3", "the draft opens on the given name");
}

/// An edit dispatched while the new-queue request still waits: every queue edit
/// pumps the link for its own answer, so the pump can meet the list and the
/// create's answer first. Those events must land in order — the list updates
/// the control's queue list, so the create's answer shows the new queue as a
/// resting queue and subscribes to it — and the create's answer must still
/// reach its request instead of being adopted as a background snapshot the
/// drain never sees (spec `gui-queue-chooser` "The new-queue control makes and
/// shows a queue").
#[test]
fn an_edit_pump_that_meets_the_create_answer_still_opens_the_new_queue() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    let tx = inject_local_events(&mut app);

    app.dispatch(&AppCommand::Panel(PanelIntent::RequestNewQueue));
    let created = sent_create(&cmd_rx).expect("the control asks for a new queue");
    // The Owner publishes the list, then answers the create with the new
    // queue's own snapshot. The stub mints the edit's `op` next, so all three
    // events are already waiting when the edit's pump starts — the ordering a
    // real Owner produces.
    tx.send(PlayerEvent::NamedQueue(mbv_ctrl::NamedQueueEvent::List {
        queues: list_with_new_queue(),
        current: current(),
    }))
    .unwrap();
    tx.send(op_answer(created.0, new_queue_snapshot())).unwrap();
    tx.send(op_answer(created.0 + 1, new_queue_with(&make_items(1))))
        .unwrap();

    let appended = app.append_on_owner(
        make_items(1)
            .into_iter()
            .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
        None,
    );

    assert!(appended, "the edit's own answer still applies");
    assert!(
        app.pending_new_queue.is_none(),
        "the create's answer ends its request"
    );
    assert_eq!(
        app.shown_entry,
        Some(ShownEntry::Local(new_queue())),
        "the Queue panel shows the queue the Owner made"
    );
    assert_eq!(
        app.shown_resting_queue_id(),
        Some(new_queue()),
        "the new queue is shown as the resting queue it is, never as the current one"
    );
    assert!(
        sent_show(&cmd_rx, new_queue()),
        "the Client subscribes to the queue it shows"
    );
    assert_eq!(
        app.displayed_queue().slots().len(),
        1,
        "the shown queue holds the appended item"
    );
    assert_eq!(
        app.local_view.slots().len(),
        3,
        "the current queue's rows are never the shown queue's"
    );
    let Some(GuiOverlay::QueueChooser(chooser)) = &app.gui.overlay else {
        panic!("the new queue's name is open for edit");
    };
    assert_eq!(
        chooser.renaming.as_ref().map(|draft| draft.queue),
        Some(new_queue())
    );

    app.dispatch(&AppCommand::Panel(PanelIntent::RequestNewQueue));

    assert_eq!(
        sent_create(&cmd_rx),
        Some(mbv_ctrl::QueueOpId(created.0 + 2)),
        "the control makes another queue once its request ended"
    );
}

/// A rejected create ends its request and tells the user why: the control
/// works again, and the press is never silent (design D8, spec
/// `gui-queue-chooser` "The new-queue control makes and shows a queue").
#[test]
fn a_rejected_create_ends_the_request_and_tells_the_user_why() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.dispatch(&AppCommand::Panel(PanelIntent::RequestNewQueue));
    let created = sent_create(&cmd_rx).expect("the control asks for a new queue");
    app.toasts.clear();

    app.handle_player_event(PlayerEvent::QueueOpResult {
        op: created,
        outcome: mbv_ctrl::QueueOpOutcome::Rejected("named queues unavailable".into()),
    });

    assert!(
        app.pending_new_queue.is_none(),
        "the refusal ends the request"
    );
    assert_eq!(
        app.newest_toast_text(),
        "named queues unavailable",
        "the refusal says why no queue was made"
    );
}

/// A rejection that answers no Named-queue command stays silent: an edit's own
/// rejection already flashed from the pump that waited for it (design D8).
#[test]
fn a_rejection_that_answers_no_named_queue_command_stays_silent() {
    let (mut app, _cmd_rx) = gui_app_with_named_queues();
    app.toasts.clear();

    app.handle_player_event(PlayerEvent::QueueOpResult {
        op: mbv_ctrl::QueueOpId(42),
        outcome: mbv_ctrl::QueueOpOutcome::Rejected("MISSING_SLOT".into()),
    });

    assert!(
        app.newest_toast_text().is_empty(),
        "nothing reports an edit's own rejection twice"
    );
}

/// Closing the current queue while it plays asks through the shared
/// confirmation first, naming the queue and the stop, and only the confirmed
/// answer sends the close (spec `gui-queue-chooser` "Close the playing queue").
#[test]
fn closing_the_playing_current_queue_asks_and_then_closes() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_local_current_entry();
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
    });

    request_close(&mut app);

    assert!(
        cmd_rx.try_recv().is_err(),
        "nothing closes before the user confirms"
    );
    let Some(OverlayRequest::Confirm(modal)) = app.pending_overlay.take() else {
        panic!("the playing current queue asks before it closes");
    };
    assert_eq!(modal.on_confirm, ConfirmAction::CloseNamedQueue(current()));
    assert!(modal.message.contains("Default"), "the prompt names it");
    assert!(
        modal.message.contains("stops playback"),
        "the prompt says that playback stops"
    );

    app.apply_confirm_action(modal.on_confirm, answer_key(Key::Enter));

    assert_eq!(
        sent_close(&cmd_rx),
        Some(current()),
        "the confirmed close names the queue"
    );
}

/// Closing the current queue while it plays nothing asks nothing: the Owner
/// deletes it and makes the first remaining queue current without playing
/// (spec `gui-queue-chooser` "Close the stopped current queue").
#[test]
fn closing_the_stopped_current_queue_closes_at_once() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_local_current_entry();
    app.player.update_status(|status| status.active = false);

    request_close(&mut app);

    assert!(
        app.pending_overlay.is_none(),
        "a stopped queue closes without a prompt"
    );
    assert_eq!(
        sent_close(&cmd_rx),
        Some(current()),
        "the close reaches the shown queue at once"
    );
}

/// Cancelling the close leaves everything as it was: no close reaches the
/// Owner and the playing queue stays current and shown (spec `gui-queue-chooser`
/// "Cancel the close").
#[test]
fn cancelling_the_close_changes_nothing() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_local_current_entry();
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
    });

    request_close(&mut app);
    let Some(OverlayRequest::Confirm(modal)) = app.pending_overlay.take() else {
        panic!("the playing current queue asks before it closes");
    };
    app.apply_confirm_action(modal.on_confirm, answer_key(Key::Esc));

    assert_eq!(sent_close(&cmd_rx), None, "no queue closes");
    assert_eq!(
        app.shown_entry,
        Some(ShownEntry::Local(current())),
        "the Queue panel still shows the playing queue"
    );
}

/// Closing a shown peer entry disconnects from the peer and sends it nothing:
/// the peer keeps playing (spec `gui-queue-chooser` "Close a peer entry").
#[test]
fn closing_a_peer_entry_disconnects_and_leaves_the_peer_playing() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = gui_app_with_peer();
    assert!(
        app.shown_entry == Some(ShownEntry::Peer),
        "the connect shows the peer entry"
    );

    request_close(&mut app);

    assert!(!app.direct_remote_connected, "the Client disconnects");
    assert_eq!(
        app.shown_entry,
        Some(ShownEntry::Local(current())),
        "the Queue panel falls back to the current local queue"
    );
    assert_eq!(sent_close(&home_cmd_rx), None, "no local queue closes");
    assert_eq!(
        sent_close(&peer_cmd_rx),
        None,
        "the peer is sent nothing, so it keeps playing"
    );
}

/// The close control does nothing while the Owner holds one local queue: the
/// last queue does not close, and the header disables the control on it (spec
/// `gui-queue-chooser` "The close control closes the shown entry").
#[test]
fn the_only_local_queue_does_not_close() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.adopt_named_queue_list(vec![named_queue_list()[0].clone()], current());
    app.show_local_current_entry();

    request_close(&mut app);

    assert_eq!(sent_close(&cmd_rx), None, "the only local queue stays");
}

/// The `resting()` queue is untouched by a close that names another queue: the
/// control closes the shown entry only, never a queue the panel does not show.
#[test]
fn a_resting_queue_shown_closes_without_a_prompt() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_named_queue(resting());

    request_close(&mut app);

    assert!(
        app.pending_overlay.is_none(),
        "a resting queue asks nothing"
    );
    assert_eq!(sent_close(&cmd_rx), Some(resting()));
}

/// Show the only local queue in the Queue panel, as a chooser press does.
fn show_the_only_local_queue(app: &mut App) {
    app.show_local_current_entry();
}

/// Show the connected peer's entry in the Queue panel, as a connect does.
fn show_the_peer_entry(app: &mut App) {
    app.show_peer_entry();
}

/// The header's close control follows the shown entry, not the chooser's entry
/// count (spec `gui-queue-chooser` "The close control closes the shown entry":
/// "The control SHALL be disabled on the only local queue"). With one local
/// queue and a connected peer the chooser lists two entries, and the control is
/// still disabled while that only local queue is shown — the close would send
/// nothing. A shown peer entry stays closable, because closing it disconnects.
#[rstest]
#[case::only_local_queue_shown(show_the_only_local_queue, true)]
#[case::peer_entry_shown(show_the_peer_entry, false)]
fn the_close_control_follows_the_shown_entry_beside_a_peer(
    #[case] show: fn(&mut App),
    #[case] disabled: bool,
) {
    let (mut app, _home_cmd_rx, _peer_cmd_rx) = gui_app_with_peer();
    app.adopt_named_queue_list(vec![named_queue_list()[0].clone()], current());
    show(&mut app);

    let header = app.gui_queue_header_projection();

    assert_eq!(
        app.queue_chooser_entries().len(),
        2,
        "one local queue and the peer entry"
    );
    assert_eq!(header.close_disabled, disabled);
}

/// A press on a Queue chooser entry shows the entry the window painted and
/// dismisses the chooser on it, and starts, stops, and moves no playback (spec
/// `gui-queue-chooser` "The chooser lists local queues and connected peers").
/// The region carries the entry's own queue, so the press shows what the
/// window painted rather than whatever the list's row holds now.
#[test]
fn a_chooser_entry_press_shows_the_painted_queue() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_local_current_entry();
    app.toggle_gui_queue_chooser();
    while cmd_rx.try_recv().is_ok() {}

    // The GUI Client's playback entry, which runs the shared effect and then
    // its own half of the chooser's entry press (design D4a (a)).
    app.handle_gui_surface_msg(Msg::Playback(PlaybackRequest::QueueChooserEntry(Some(
        resting(),
    ))));

    assert_eq!(
        app.shown_entry,
        Some(ShownEntry::Local(resting())),
        "the press shows the entry it painted"
    );
    assert!(
        !app.gui.overlay_is_open(),
        "the chooser dismisses on the entry it showed"
    );
    assert!(
        matches!(cmd_rx.try_recv(), Ok(CtrlCmd::NamedQueueShow { queue }) if queue == resting()),
        "the shown queue's snapshots are asked of the local Owner"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "the show sends no playback command"
    );
}

/// A press on a Queue chooser entry whose queue the Owner no longer holds —
/// another Client closed it between the paint and the press — shows the current
/// local queue instead, so the panel never shows a queue with no name and no
/// edits (spec `gui-queue-chooser` "The close control closes the shown entry":
/// after a close the Queue panel shows the current local queue).
#[test]
fn a_chooser_entry_press_on_a_closed_queue_shows_the_current_queue() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_named_queue(resting());
    app.toggle_gui_queue_chooser();
    app.adopt_named_queue_list(vec![named_queue_list()[0].clone()], current());
    while cmd_rx.try_recv().is_ok() {}

    app.handle_gui_surface_msg(Msg::Playback(PlaybackRequest::QueueChooserEntry(Some(
        resting(),
    ))));

    assert_eq!(
        app.shown_entry,
        Some(ShownEntry::Local(current())),
        "the panel falls back to the current local queue"
    );
    assert!(
        !app.gui.overlay_is_open(),
        "the chooser dismisses on the entry it pressed"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "the closed queue is asked for nothing"
    );
}
