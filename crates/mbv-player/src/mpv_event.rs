//! Domain view of the mpv events the playback run loop acts on.
//!
//! The run loop matches on [`MpvEvent`], not on the JSON IPC line shape, so
//! event classification does not depend on how an event was delivered.

use std::fmt;

/// The text that mpv puts in `file_error` when it cannot initialize the audio
/// output (`MPV_ERROR_AO_INIT_FAILED`). The IPC line carries text, not an
/// error code, so the decoder types the failure from this text.
pub(crate) const AUDIO_OUTPUT_INIT_FAILED: &str = "audio output initialization failed";

mod ipc;
#[cfg(test)]
mod tests;

/// Why mpv left a playlist entry without an error.
///
/// mpv's `error` reason is not here: an end-file with an error arrives as
/// [`MpvEvent::Failed`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EndFileReason {
    Eof,
    Stop,
    Quit,
    Redirect,
}

/// The value of an observed property, in the format that `observe_properties`
/// requests for it.
#[derive(Debug)]
pub(crate) enum PropertyValue {
    Double(f64),
    Flag(bool),
    Int64(i64),
    Str(String),
    /// The property is unavailable. The IPC reports this as a change with no
    /// data.
    None,
}

/// An mpv failure: an end-file with an error, or a failed event.
#[derive(Debug)]
pub(crate) enum MpvFailure {
    AudioOutputInit,
    /// The child process closed the IPC socket with no quit requested (S8).
    ProcessExited,
    Other(String),
}

impl fmt::Display for MpvFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AudioOutputInit => f.write_str(AUDIO_OUTPUT_INIT_FAILED),
            Self::ProcessExited => f.write_str("mpv process exited"),
            Self::Other(text) => f.write_str(text),
        }
    }
}

#[derive(Debug)]
pub(crate) enum MpvEvent {
    PropertyChange {
        name: String,
        value: PropertyValue,
    },
    PlaybackRestart,
    EndFile(EndFileReason),
    Failed(MpvFailure),
    LogMessage {
        prefix: String,
        level: String,
        text: String,
    },
    ClientMessage(Vec<String>),
    Shutdown,
    /// An event the run loop does not act on.
    Ignored,
}
