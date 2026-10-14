//! Terminal input port for the `TuiRealm` listener. It folds each run of
//! queued mouse moves into its newest event (issue #940). `EnableMouseCapture`
//! reports every pointer move, and an unfolded burst would queue ahead of keys
//! and clicks, one event per main-loop pass.

use std::io;
use std::time::Duration;

use crossterm::event::{self as xterm, Event as XtermEvent, MouseEventKind};
use mbv_ui_msg::UserEvent;
use tuirealm::event::Event;
use tuirealm::listener::{Poll, PortError, PortResult};

/// Crossterm input port that coalesces consecutive `Moved` events.
pub(super) struct CoalescingInputPort {
    /// Blocking poll timeout: half the listener interval, as tuirealm's
    /// built-in crossterm listener uses.
    timeout: Duration,
    /// A non-move event read ahead of time. The next `poll` returns it first.
    pending: Option<XtermEvent>,
}

impl CoalescingInputPort {
    pub(super) fn new(interval: Duration) -> Self {
        Self {
            timeout: interval / 2,
            pending: None,
        }
    }
}

impl Poll<UserEvent> for CoalescingInputPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        let first = match self.pending.take() {
            Some(event) => event,
            None => match read_blocking(self.timeout)? {
                Some(event) => event,
                None => return Ok(None),
            },
        };
        let (deliver, stash) = coalesce_moves(first, read_ready);
        self.pending = stash;
        Ok(Some(Event::from(deliver)))
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

/// Read one event only if input is already ready. A failed read ends the run
/// and is logged. The next blocking poll reads the same terminal, so a
/// persistent failure still reaches tuirealm.
fn read_ready() -> Option<XtermEvent> {
    match read_blocking(Duration::ZERO) {
        Ok(event) => event,
        Err(error) => {
            tracing::warn!(
                name: "terminal_input.coalesce.read_failed",
                target: "terminal_input",
                error = %error,
                "terminal read failed while coalescing mouse moves"
            );
            None
        }
    }
}

/// Decide what one `poll` delivers. `first` is the event just read. `next_ready`
/// yields only input that is already ready. Returns the event to deliver and,
/// when a non-move event ended the run, that event for the next `poll`.
fn coalesce_moves(
    first: XtermEvent,
    mut next_ready: impl FnMut() -> Option<XtermEvent>,
) -> (XtermEvent, Option<XtermEvent>) {
    if !is_moved(&first) {
        return (first, None);
    }
    let mut held = first;
    loop {
        match next_ready() {
            None => return (held, None),
            Some(next) if is_moved(&next) => held = next,
            Some(other) => return (held, Some(other)),
        }
    }
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

    /// #940: a run of mouse moves collapses to the newest move.
    #[test]
    fn run_of_moves_delivers_only_the_last() {
        let mut ready = vec![moved(2), moved(3)].into_iter();
        assert_eq!(coalesce_moves(moved(1), || ready.next()), (moved(3), None));
    }

    /// #940: a key read behind a move is stashed for the next poll, not dropped.
    #[test]
    fn move_followed_by_key_delivers_move_and_stashes_key() {
        let mut ready = vec![key()].into_iter();
        assert_eq!(
            coalesce_moves(moved(1), || ready.next()),
            (moved(1), Some(key()))
        );
    }

    /// #940: non-move input passes through unchanged, with no read-ahead.
    #[test]
    fn non_move_first_event_is_delivered_without_reading_ahead() {
        assert_eq!(
            coalesce_moves(key(), || panic!("source must not be read")),
            (key(), None)
        );
    }
}
