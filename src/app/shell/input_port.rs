//! Terminal input port for the `TuiRealm` listener. It keeps at most one
//! mouse move in flight toward the main loop (issue #940). `EnableMouseCapture`
//! reports every pointer move. The listener thread reads far faster than the
//! main loop (one event, one full sync and redraw per pass), so forwarding each
//! move would grow the unbounded channel and queue keys and clicks behind it.
//!
//! The port forwards a `Moved` only while the shared [`MoveGate`] is open. While
//! it is closed, newer moves replace the held one. The main loop opens the gate
//! after every terminal tick, so the held move then goes out with the latest
//! pointer position.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self as xterm, Event as XtermEvent, MouseEventKind};
use mbv_ui_msg::UserEvent;
use tuirealm::event::Event;
use tuirealm::listener::{Poll, PortError, PortResult};

/// Most terminal events one `poll` folds. Continuous motion cannot hold a move
/// back past this many reads.
const MAX_FOLD: usize = 256;

/// Shared flag for "a `Moved` is on its way to the main loop". The port closes
/// it when it forwards a move. The main loop opens it after each terminal tick,
/// whatever event that tick took. A tick that took another event can open the
/// gate while a move is still queued. The queue then holds at most one extra
/// move per user event, so it stays bounded.
#[derive(Clone, Default)]
pub(super) struct MoveGate(Arc<AtomicBool>);

impl MoveGate {
    /// The main loop calls this after each terminal tick.
    pub(super) fn release(&self) {
        self.0.store(false, Ordering::Release);
    }

    /// Close the gate. Returns `true` when it was open, so the caller may forward.
    fn try_close(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn close(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// Crossterm input port that holds back all but the newest `Moved` event.
pub(super) struct CoalescingInputPort {
    /// Blocking poll timeout: half the listener interval, as tuirealm's
    /// built-in crossterm listener uses.
    timeout: Duration,
    gate: MoveGate,
    /// The newest move the closed gate kept back.
    held_move: Option<XtermEvent>,
    /// A non-move event read ahead of time. The next `poll` returns it first.
    stash: Option<XtermEvent>,
    /// A read error met after events were already folded. The next `poll`
    /// returns it, so tuirealm still sees a permanent error.
    deferred_error: Option<PortError>,
}

impl CoalescingInputPort {
    pub(super) fn new(interval: Duration, gate: MoveGate) -> Self {
        Self {
            timeout: interval / 2,
            gate,
            held_move: None,
            stash: None,
            deferred_error: None,
        }
    }

    /// Choose the one event this poll delivers from a folded run.
    fn route(&mut self, folded: FoldedRun) -> Option<XtermEvent> {
        self.deferred_error = folded.error;
        match (folded.latest_move, folded.other) {
            (None, other) => other,
            // A key or click must not overtake the move before it. This forward
            // ignores the gate. The user event bounds the extra move.
            (Some(moved), Some(other)) => {
                self.gate.close();
                self.stash = Some(other);
                Some(moved)
            }
            (Some(moved), None) if self.gate.try_close() => Some(moved),
            (Some(moved), None) => {
                self.held_move = Some(moved);
                None
            }
        }
    }
}

impl Poll<UserEvent> for CoalescingInputPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        if let Some(event) = self.stash.take() {
            return Ok(Some(Event::from(event)));
        }
        if let Some(error) = self.deferred_error.take() {
            return Err(error);
        }
        let first = read_blocking(self.timeout)?;
        let folded = fold_run(self.held_move.take(), first, read_ready);
        Ok(self.route(folded).map(Event::from))
    }
}

/// Read one event, waiting at most `timeout`. `Ok(None)` means no input arrived.
fn read_blocking(timeout: Duration) -> PortResult<Option<XtermEvent>> {
    if xterm::poll(timeout).map_err(|err| io_err_to_port_err(&err))? {
        xterm::read()
            .map(Some)
            .map_err(|err| io_err_to_port_err(&err))
    } else {
        Ok(None)
    }
}

/// Read one event only if input is already ready.
fn read_ready() -> PortResult<Option<XtermEvent>> {
    read_blocking(Duration::ZERO)
}

/// What one pass over the ready input found.
struct FoldedRun {
    /// The newest move seen, including a move held from an earlier poll.
    latest_move: Option<XtermEvent>,
    /// The first non-move event. It ended the run.
    other: Option<XtermEvent>,
    /// A permanent read error that ended the run early.
    error: Option<PortError>,
}

/// Fold `first` and the input `next_ready` yields into the newest move plus the
/// first non-move event. `held` is a move kept back by an earlier poll. Reads at
/// most [`MAX_FOLD`] events. `next_ready` yields only input that is already ready.
fn fold_run(
    held: Option<XtermEvent>,
    first: Option<XtermEvent>,
    mut next_ready: impl FnMut() -> PortResult<Option<XtermEvent>>,
) -> FoldedRun {
    let mut run = FoldedRun {
        latest_move: held,
        other: None,
        error: None,
    };
    let mut incoming = first;
    let mut reads = 0;
    while let Some(event) = incoming {
        if is_moved(&event) {
            run.latest_move = Some(event);
        } else {
            run.other = Some(event);
            break;
        }
        reads += 1;
        if reads == MAX_FOLD {
            break;
        }
        match next_ready() {
            Ok(next) => incoming = next,
            Err(error @ PortError::PermanentError(_)) => {
                run.error = Some(error);
                break;
            }
            Err(error @ PortError::IntermittentError(_)) => {
                tracing::warn!(
                    name: "terminal_input.coalesce.read_failed",
                    target: "terminal_input",
                    error = %error,
                    "terminal read failed while folding mouse moves"
                );
                break;
            }
        }
    }
    run
}

fn is_moved(event: &XtermEvent) -> bool {
    matches!(
        event,
        XtermEvent::Mouse(mouse) if mouse.kind == MouseEventKind::Moved
    )
}

/// Same mapping as tuirealm's built-in crossterm listener. A permanent error
/// stops the port, which is how a closed terminal ends the main loop.
fn io_err_to_port_err(err: &io::Error) -> PortError {
    use io::ErrorKind;

    match err.kind() {
        ErrorKind::Interrupted
        | ErrorKind::NetworkDown
        | ErrorKind::ResourceBusy
        | ErrorKind::ConnectionReset
        | ErrorKind::TimedOut
        | ErrorKind::QuotaExceeded
        | ErrorKind::StorageFull
        | ErrorKind::WouldBlock => PortError::IntermittentError(err.to_string()),
        _ => PortError::PermanentError(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};

    use super::*;

    fn moved(column: u16) -> XtermEvent {
        XtermEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column,
            row: 0,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn key() -> XtermEvent {
        XtermEvent::Key(KeyEvent::from(KeyCode::Char('q')))
    }

    fn port() -> CoalescingInputPort {
        CoalescingInputPort::new(Duration::from_millis(8), MoveGate::default())
    }

    fn run(latest_move: Option<XtermEvent>, other: Option<XtermEvent>) -> FoldedRun {
        FoldedRun {
            latest_move,
            other,
            error: None,
        }
    }

    /// #940: a run of mouse moves collapses to the newest move.
    #[test]
    fn run_of_moves_folds_to_the_last() {
        let mut ready = vec![Ok(Some(moved(2))), Ok(Some(moved(3))), Ok(None)].into_iter();
        let folded = fold_run(None, Some(moved(1)), || ready.next().unwrap());
        assert_eq!((folded.latest_move, folded.other), (Some(moved(3)), None));
    }

    /// #940: a key read behind a move ends the run and is kept, not dropped.
    #[test]
    fn key_behind_move_ends_the_run_and_is_kept() {
        let mut ready = vec![Ok(Some(key()))].into_iter();
        let folded = fold_run(None, Some(moved(1)), || ready.next().unwrap());
        assert_eq!(
            (folded.latest_move, folded.other),
            (Some(moved(1)), Some(key()))
        );
    }

    /// #940: non-move input passes through with no read-ahead.
    #[test]
    fn non_move_first_event_is_not_read_past() {
        let folded = fold_run(None, Some(key()), || panic!("source must not be read"));
        assert_eq!((folded.latest_move, folded.other), (None, Some(key())));
    }

    /// #940: a move held by the closed gate is the base for the next run.
    #[test]
    fn held_move_is_kept_when_no_input_arrives() {
        let folded = fold_run(Some(moved(4)), None, || panic!("source must not be read"));
        assert_eq!(folded.latest_move, Some(moved(4)));
    }

    /// #940: continuous motion cannot hold a move back without bound.
    #[test]
    fn endless_moves_stop_after_the_fold_cap() {
        let reads = Cell::new(0);
        let folded = fold_run(None, Some(moved(1)), || {
            reads.set(reads.get() + 1);
            Ok(Some(moved(2)))
        });
        assert_eq!(
            (folded.latest_move, reads.get()),
            (Some(moved(2)), MAX_FOLD - 1)
        );
    }

    /// #940: a read error mid-run is kept for tuirealm, with the move still folded.
    #[test]
    fn read_error_mid_run_is_kept() {
        let folded = fold_run(None, Some(moved(1)), || {
            Err(PortError::PermanentError("hup".to_owned()))
        });
        assert_eq!(
            (folded.latest_move, folded.error),
            (
                Some(moved(1)),
                Some(PortError::PermanentError("hup".to_owned()))
            )
        );
    }

    /// #940: a transient read error ends the fold without a deferred error.
    #[test]
    fn intermittent_read_error_mid_run_is_not_deferred() {
        let folded = fold_run(None, Some(moved(1)), || {
            Err(PortError::IntermittentError("interrupted".to_owned()))
        });
        assert_eq!(folded.error, None);
    }

    /// #940: an open gate forwards the move and closes behind it.
    #[test]
    fn open_gate_forwards_the_move_and_closes() {
        let mut port = port();
        assert_eq!(port.route(run(Some(moved(1)), None)), Some(moved(1)));
        assert!(!port.gate.try_close());
    }

    /// #940: a closed gate keeps the move back for a later poll.
    #[test]
    fn closed_gate_holds_the_move() {
        let mut port = port();
        port.gate.close();
        assert_eq!(port.route(run(Some(moved(1)), None)), None);
        assert_eq!(port.held_move, Some(moved(1)));
    }

    /// #940: a released gate lets the held move go out.
    #[test]
    fn released_gate_forwards_the_held_move() {
        let mut port = port();
        port.gate.close();
        port.gate.release();
        assert_eq!(port.route(run(Some(moved(1)), None)), Some(moved(1)));
    }

    /// #940: a key behind a held move goes out after the move, gate or not.
    #[test]
    fn key_never_overtakes_the_move_before_it() {
        let mut port = port();
        port.gate.close();
        assert_eq!(port.route(run(Some(moved(1)), Some(key()))), Some(moved(1)));
        assert_eq!(port.stash, Some(key()));
    }
}
