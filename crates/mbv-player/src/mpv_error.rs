//! Domain error for the mpv backend.
//!
//! `PlayerError` wraps this instead of the backend's own error type, so the
//! backend can change (libmpv today, the IPC child process later) without
//! changing `PlayerError`'s shape.
//!
//! The change `mpv-child-process` grows this enum with the backend: the IPC
//! session adds `Spawn`, `NotFound`, `Io`, `Reply`, `Timeout`, `Closed`, and
//! `Decode` when its code starts to produce them, and group 4 removes
//! `Libmpv` with the libmpv backend.

use std::error::Error;
use std::fmt;

use crate::mpv_err_str;

/// Errors raised by the mpv backend.
#[derive(Debug)]
pub(crate) enum MpvError {
    /// A libmpv call failed; the text comes from `mpv_err_str`. This case goes
    /// away when the backend changes to the IPC child process.
    Libmpv(String),
}

impl fmt::Display for MpvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Libmpv(text) => f.write_str(text),
        }
    }
}

impl Error for MpvError {}

impl From<libmpv2::Error> for MpvError {
    fn from(error: libmpv2::Error) -> Self {
        Self::Libmpv(mpv_err_str(&error))
    }
}
