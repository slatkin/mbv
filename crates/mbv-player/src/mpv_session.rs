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

use serde_json::{Value, json};

use crate::mpv_error::MpvError;
use crate::mpv_event::{MpvEvent, PropertyValue};
use crate::mpv_session::ipc::IpcSession;
use crate::mpv_session::spawn::Child;

/// How long one request waits for mpv's reply before the session treats the
/// child as hung (design D3). A `request` that passes this bound kills nothing
/// itself; the run loop's `next_event` turns the timeout into a failure event.
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

impl From<i64> for PropertyValue {
    fn from(value: i64) -> Self {
        Self::Int64(value)
    }
}

impl From<bool> for PropertyValue {
    fn from(value: bool) -> Self {
        Self::Flag(value)
    }
}

impl From<f64> for PropertyValue {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl From<&str> for PropertyValue {
    fn from(value: &str) -> Self {
        Self::Str(value.to_owned())
    }
}

impl From<String> for PropertyValue {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
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
}

impl Mpv {
    /// Wraps a started child: duplicates its connected socket into a session
    /// and keeps the child so it lives and dies with the session (design D5).
    pub(crate) fn from_child(child: Child) -> Result<Self, MpvError> {
        let stream = child.try_clone_stream()?;
        let session = IpcSession::from_stream(stream)?;
        Ok(Self {
            _child: Some(child),
            session: RefCell::new(session),
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
        }
    }

    /// Sets a property from the domain value type (design D2). The JSON type
    /// matches the `PropertyValue` variant, and mpv converts it for the
    /// property's own type.
    pub(crate) fn set_property(
        &self,
        name: &str,
        value: impl Into<PropertyValue>,
    ) -> Result<(), MpvError> {
        let Some(value) = property_json(value.into()) else {
            // Documented no-op: an unavailable property carries no value to
            // set; only the observed-change decoder builds `None`.
            return Ok(());
        };
        self.session
            .borrow_mut()
            .request_values(
                &[json!("set_property"), json!(name), value],
                REQUEST_TIMEOUT,
            )
            .map(|_| ())
    }

    pub(crate) fn get_f64(&self, name: &str) -> Result<f64, MpvError> {
        let data = self.get_property(name)?;
        data.as_f64()
            .ok_or_else(|| MpvError::Reply(format!("property `{name}` is not a double")))
    }

    pub(crate) fn get_i64(&self, name: &str) -> Result<i64, MpvError> {
        let data = self.get_property(name)?;
        data.as_i64()
            .ok_or_else(|| MpvError::Reply(format!("property `{name}` is not an integer")))
    }

    pub(crate) fn get_bool(&self, name: &str) -> Result<bool, MpvError> {
        let data = self.get_property(name)?;
        data.as_bool()
            .ok_or_else(|| MpvError::Reply(format!("property `{name}` is not a flag")))
    }

    pub(crate) fn get_string(&self, name: &str) -> Result<String, MpvError> {
        let data = self.get_property(name)?;
        data.as_str()
            .map(str::to_owned)
            .ok_or_else(|| MpvError::Reply(format!("property `{name}` is not a string")))
    }

    /// Sends a command and waits for mpv to accept it. A command that fails
    /// while it runs reports through events, not here.
    pub(crate) fn command(&self, name: &str, args: &[&str]) -> Result<(), MpvError> {
        let mut command = Vec::with_capacity(args.len() + 1);
        command.push(name);
        command.extend_from_slice(args);
        self.session
            .borrow_mut()
            .request(&command, REQUEST_TIMEOUT)
            .map(|_| ())
    }

    /// Queues `name` without waiting for its reply. The backend reports only
    /// a synchronous send failure here; command failures arrive as events.
    pub(crate) fn command_async(&self, name: &str, args: &[&str]) -> Result<(), MpvError> {
        let mut command = Vec::with_capacity(args.len() + 1);
        command.push(name);
        command.extend_from_slice(args);
        self.session.borrow_mut().send(&command)
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
        self.session
            .borrow_mut()
            .request_values(&[json!(command), json!(id), json!(name)], REQUEST_TIMEOUT)
            .map(|_| ())
    }

    /// Requests log messages at or above `level` (an mpv level name, for
    /// example `warn`), so they arrive as log-message events.
    pub(crate) fn request_log_messages(&self, level: &str) -> Result<(), MpvError> {
        self.session
            .borrow_mut()
            .request(&["request_log_messages", level], REQUEST_TIMEOUT)
            .map(|_| ())
    }

    /// Pops the next event, or `None` when the child has none now. The IPC
    /// session queues events that arrive while a request waits for its reply.
    pub(crate) fn next_event(&self) -> Option<MpvEvent> {
        self.session.borrow_mut().next_event()
    }

    /// The raw descriptor of the child's IPC socket, so the run loop can poll
    /// it together with the wakeup pipe (design D3).
    pub(crate) fn socket_fd(&self) -> std::os::unix::io::RawFd {
        self.session.borrow().socket_fd()
    }

    fn get_property(&self, name: &str) -> Result<Value, MpvError> {
        self.session
            .borrow_mut()
            .request(&["get_property", name], REQUEST_TIMEOUT)
    }
}

/// The JSON value of a set-property argument, or `None` for an unavailable
/// property (which sets nothing).
fn property_json(value: PropertyValue) -> Option<Value> {
    match value {
        PropertyValue::Double(value) => Some(json!(value)),
        PropertyValue::Flag(value) => Some(json!(value)),
        PropertyValue::Int64(value) => Some(json!(value)),
        PropertyValue::Str(value) => Some(Value::String(value)),
        PropertyValue::None => None,
    }
}
