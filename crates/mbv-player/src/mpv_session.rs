//! Crate-private mpv session wrapper (design D2 of `mpv-child-process`).
//!
//! [`Mpv`] is the only mpv-handle type the player names outside this module
//! and the event types (`mpv_event.rs`). It wraps the live backend — the
//! `mpv` child process that mbv controls over its JSON IPC socket — and offers
//! only the calls the player makes.

pub(crate) mod ipc;
pub(crate) mod spawn;

use std::cell::RefCell;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::mpv_error::MpvError;
use crate::mpv_event::MpvEvent;
use crate::mpv_session::ipc::IpcSession;
use crate::mpv_session::spawn::{Child, Started};

/// How long one request waits for mpv's reply before the session treats the
/// child as hung (design D3). A `request` that passes this bound returns
/// `MpvError::Timeout` and the session queues the child's end event, so the
/// next `next_event` ends the run and its `Drop` kills the child.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The format of an observed property (design D2 and D8). The IPC backend
/// maps it onto an mpv observation command: a string property uses
/// `observe_property_string`, every other format uses `observe_property`.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Format {
    Double,
    Flag,
    Int64,
    String,
}

/// The player's one mpv session. One playback run owns one session, and one
/// thread owns the session. The session owns the child, so the child lives
/// exactly as long as the run (design D5); the socket lives in a `RefCell`
/// because the player's read and write calls all take `&self`.
pub(crate) struct Mpv {
    /// Owns the child process. The field is never read: it exists so the
    /// child lives exactly as long as the session and its `Drop` kills and
    /// reaps the process (design D5). It is `None` only in tests, which build
    /// the session over a socket pair and start no child (design D9).
    _child: Option<Child>,
    session: RefCell<IpcSession>,
    /// How long a request waits for its reply (design D3). Production uses
    /// `REQUEST_TIMEOUT`; the test constructor passes zero.
    request_timeout: Duration,
}

impl Mpv {
    /// Wraps a started child: drives its connected socket in a session and
    /// keeps the child so it lives and dies with the session (design D5).
    pub(crate) fn from_child((child, stream): Started) -> Result<Self, MpvError> {
        let session = IpcSession::from_stream(stream)?;
        Ok(Self {
            _child: Some(child),
            session: RefCell::new(session),
            request_timeout: REQUEST_TIMEOUT,
        })
    }

    /// A session over an already-connected socket and no child, for tests that
    /// exercise the run loop's event handling without starting mpv (design
    /// D9).
    #[cfg(test)]
    pub(crate) fn for_test(session: IpcSession) -> Self {
        Self {
            _child: None,
            session: RefCell::new(session),
            // A zero timeout makes a request against a silent peer time out
            // at once, so the test needs no wait (design D9).
            request_timeout: Duration::ZERO,
        }
    }

    /// Sets a property (design D2). The JSON type of `value` is the type that
    /// mpv converts for the property's own type.
    pub(crate) fn set_property(&self, name: &str, value: impl Into<Value>) -> Result<(), MpvError> {
        let result = self.session.borrow_mut().request_values(
            &[json!("set_property"), json!(name), value.into()],
            self.request_timeout,
        );
        result.map(|_| ())
    }

    /// Reads a property as `T`. A value of another JSON type is a decode error.
    pub(crate) fn get_property<T: DeserializeOwned>(&self, name: &str) -> Result<T, MpvError> {
        let result = self
            .session
            .borrow_mut()
            .request(&["get_property", name], self.request_timeout);
        serde_json::from_value(result?)
            .map_err(|error| MpvError::Decode(format!("property `{name}`: {error}")))
    }

    /// Sends a command and waits for mpv to accept it. A command that fails
    /// while it runs reports through events, not here.
    pub(crate) fn command(&self, name: &str, args: &[&str]) -> Result<(), MpvError> {
        let result = self
            .session
            .borrow_mut()
            .request(&command_line(name, args), self.request_timeout);
        result.map(|_| ())
    }

    /// Queues `name` without waiting for its reply. The backend reports only
    /// a synchronous send failure here; command failures arrive as events.
    pub(crate) fn command_async(&self, name: &str, args: &[&str]) -> Result<(), MpvError> {
        self.session.borrow_mut().send(&command_line(name, args))
    }

    /// Observes `name` for changes; each change arrives as a
    /// [`MpvEvent::PropertyChange`] carrying the value in `format`.
    pub(crate) fn observe_property(
        &self,
        name: &str,
        id: u64,
        format: Format,
    ) -> Result<(), MpvError> {
        let command = match format {
            Format::String => "observe_property_string",
            Format::Double | Format::Flag | Format::Int64 => "observe_property",
        };
        let result = self.session.borrow_mut().request_values(
            &[json!(command), json!(id), json!(name)],
            self.request_timeout,
        );
        result.map(|_| ())
    }

    /// Requests log messages at or above `level` (an mpv level name, for
    /// example `warn`), so they arrive as log-message events.
    pub(crate) fn request_log_messages(&self, level: &str) -> Result<(), MpvError> {
        let result = self
            .session
            .borrow_mut()
            .request(&["request_log_messages", level], self.request_timeout);
        result.map(|_| ())
    }

    /// Pops the next event, or `None` when the child has none now. The IPC
    /// session owns the child's liveness, so a timed-out request queues the
    /// matching end event (design D3). The IPC session queues events that
    /// arrive while a request waits for its reply.
    pub(crate) fn next_event(&self) -> Option<MpvEvent> {
        self.session.borrow_mut().next_event()
    }

    /// The raw descriptor of the child's IPC socket, so the run loop can poll
    /// it together with the wakeup pipe (design D3).
    pub(crate) fn socket_fd(&self) -> std::os::unix::io::RawFd {
        self.session.borrow().socket_fd()
    }
}

/// The command name followed by its arguments, as one argument list.
fn command_line<'a>(name: &'a str, args: &[&'a str]) -> Vec<&'a str> {
    let mut command = Vec::with_capacity(args.len() + 1);
    command.push(name);
    command.extend_from_slice(args);
    command
}
