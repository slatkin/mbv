//! Domain view of the mpv events the playback run loop acts on.
//!
//! The run loop matches on [`MpvEvent`], not on `libmpv2::Event`, so event
//! classification does not depend on the binding that delivers the event.

use std::fmt;

use libmpv2::events::{Event, PropertyData};
use libmpv2::mpv_end_file_reason;

use super::mpv_err_str;

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
    /// The property is unavailable. JSON IPC reports this as a change with no
    /// data. libmpv2 swallows such a change, so only the IPC decoder builds it.
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
            Self::AudioOutputInit => f.write_str(&mpv_err_str(&libmpv2::Error::Raw(
                libmpv2::mpv_error::AoInitFailed,
            ))),
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

impl MpvEvent {
    /// libmpv2 returns an end-file that carries an error code as `Err`, so an
    /// end-file error becomes [`MpvEvent::Failed`], not [`MpvEvent::EndFile`].
    pub(crate) fn from_libmpv(event: Result<Event<'_>, libmpv2::Error>) -> Self {
        match event {
            Ok(Event::PropertyChange { name, change, .. }) => Self::PropertyChange {
                name: name.to_owned(),
                value: PropertyValue::from_libmpv(&change),
            },
            Ok(Event::PlaybackRestart) => Self::PlaybackRestart,
            Ok(Event::EndFile(reason)) => end_file_from_libmpv(reason),
            Ok(Event::LogMessage {
                prefix,
                level,
                text,
                ..
            }) => Self::LogMessage {
                prefix: prefix.to_owned(),
                level: level.to_owned(),
                text: text.to_owned(),
            },
            Ok(Event::ClientMessage(args)) => {
                Self::ClientMessage(args.into_iter().map(str::to_owned).collect())
            }
            Ok(Event::Shutdown) => Self::Shutdown,
            Ok(
                Event::GetPropertyReply { .. }
                | Event::SetPropertyReply(_)
                | Event::CommandReply(_)
                | Event::StartFile
                | Event::FileLoaded
                | Event::VideoReconfig
                | Event::AudioReconfig
                | Event::Seek
                | Event::QueueOverflow
                | Event::Deprecated(_),
            ) => Self::Ignored,
            Err(libmpv2::Error::Raw(libmpv2::mpv_error::AoInitFailed)) => {
                Self::Failed(MpvFailure::AudioOutputInit)
            }
            Err(e) => Self::Failed(MpvFailure::Other(mpv_err_str(&e))),
        }
    }
}

/// mpv always sets an error code with the `error` reason, so libmpv2 never
/// delivers it here. Any reason without a domain variant is a failure.
fn end_file_from_libmpv(reason: libmpv2::EndFileReason) -> MpvEvent {
    match reason {
        mpv_end_file_reason::Eof => MpvEvent::EndFile(EndFileReason::Eof),
        mpv_end_file_reason::Stop => MpvEvent::EndFile(EndFileReason::Stop),
        mpv_end_file_reason::Quit => MpvEvent::EndFile(EndFileReason::Quit),
        mpv_end_file_reason::Redirect => MpvEvent::EndFile(EndFileReason::Redirect),
        other => MpvEvent::Failed(MpvFailure::Other(format!("end-file reason {other}"))),
    }
}

impl PropertyValue {
    fn from_libmpv(data: &PropertyData<'_>) -> Self {
        match *data {
            PropertyData::Double(value) => Self::Double(value),
            PropertyData::Flag(value) => Self::Flag(value),
            PropertyData::Int64(value) => Self::Int64(value),
            PropertyData::Str(value) | PropertyData::OsdStr(value) => Self::Str(value.to_owned()),
        }
    }
}
