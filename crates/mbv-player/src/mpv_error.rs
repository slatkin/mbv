//! Domain error for the mpv backend.
//!
//! `PlayerError` wraps this instead of the backend's own error type, so the
//! backend can change (libmpv today, the IPC child process later) without
//! changing `PlayerError`'s shape.
//!
//! The change `mpv-child-process` grows this enum with the backend. The IPC
//! session adds `Io`, `Reply`, `Timeout`, `Closed`, and `Decode`, and the
//! child start adds `Spawn` and `NotFound`, each with its first producer.
//! Group 4 removes `Libmpv` with the libmpv backend.

use std::error::Error;
use std::fmt;

use crate::mpv_err_str;

/// Errors raised by the mpv backend.
#[derive(Debug)]
pub(crate) enum MpvError {
    /// The `mpv` child process could not be started. The text names the fault.
    Spawn(String),
    /// The `mpv` program or its IPC socket path does not exist. The text names
    /// what was missing.
    NotFound(String),
    /// A libmpv call failed; the text comes from `mpv_err_str`. This case goes
    /// away when the backend changes to the IPC child process.
    Libmpv(String),
    /// A read or write on the IPC socket failed.
    Io(std::io::Error),
    /// mpv answered a command with an error; the text is mpv's error name.
    Reply(String),
    /// mpv did not answer a command within the request timeout.
    Timeout,
    /// The IPC socket reached end-of-file before the awaited reply arrived.
    Closed,
    /// A line from the IPC socket could not be decoded; the text says why.
    Decode(String),
}

impl fmt::Display for MpvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(text) | Self::NotFound(text) | Self::Libmpv(text) => f.write_str(text),
            Self::Io(error) => write!(f, "mpv IPC socket I/O failed: {error}"),
            Self::Reply(text) => write!(f, "mpv rejected a command: {text}"),
            Self::Timeout => f.write_str("mpv did not reply in time"),
            Self::Closed => f.write_str("mpv closed the IPC socket"),
            Self::Decode(text) => write!(f, "mpv IPC line does not decode: {text}"),
        }
    }
}

impl Error for MpvError {}

impl From<libmpv2::Error> for MpvError {
    fn from(error: libmpv2::Error) -> Self {
        Self::Libmpv(mpv_err_str(&error))
    }
}
