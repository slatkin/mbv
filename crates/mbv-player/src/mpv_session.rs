//! Crate-private mpv session wrapper (design D2 of `mpv-child-process`).
//!
//! [`Mpv`] is the only mpv-handle type the player names outside this module
//! and the event types (`mpv_event.rs`). It wraps the live backend — libmpv
//! today, the IPC child process from group 3 on — and offers only the calls
//! the player makes. The backend swap (group 4) changes this module's inside
//! and leaves every call site untouched.

pub(crate) mod spawn;

use std::ffi::CString;

use libmpv2::Mpv as Libmpv;

use crate::mpv_error::MpvError;
use crate::mpv_event::{MpvEvent, PropertyValue};

/// The format of an observed property (design D2). The backend maps it onto
/// its own observation format.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Format {
    Double,
    Flag,
    Int64,
    String,
}

impl From<Format> for libmpv2::Format {
    fn from(format: Format) -> Self {
        match format {
            Format::Double => libmpv2::Format::Double,
            Format::Flag => libmpv2::Format::Flag,
            Format::Int64 => libmpv2::Format::Int64,
            Format::String => libmpv2::Format::String,
        }
    }
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
/// thread owns the session, as the libmpv handle does today.
pub(crate) struct Mpv {
    inner: Libmpv,
}

impl Mpv {
    /// Builds a session through libmpv's initializer. The option list is the
    /// one `init_mpv` sets; the child-process backend turns it into command
    /// line arguments (design D4) and drops this constructor.
    pub(crate) fn with_initializer<F>(initializer: F) -> Result<Self, MpvError>
    where
        F: FnOnce(libmpv2::MpvInitializer) -> Result<(), libmpv2::Error>,
    {
        Libmpv::with_initializer(initializer)
            .map(|inner| Self { inner })
            .map_err(MpvError::from)
    }

    /// Sets a property from the domain value type (design D2).
    pub(crate) fn set_property(
        &self,
        name: &str,
        value: impl Into<PropertyValue>,
    ) -> Result<(), MpvError> {
        match value.into() {
            PropertyValue::Double(value) => self.inner.set_property(name, value),
            PropertyValue::Flag(value) => self.inner.set_property(name, value),
            PropertyValue::Int64(value) => self.inner.set_property(name, value),
            PropertyValue::Str(value) => self.inner.set_property(name, value),
            #[cfg(test)]
            // Documented no-op: an unavailable property carries no value to
            // set; only the observed-change decoder builds `None`.
            PropertyValue::None => Ok(()),
        }
        .map_err(MpvError::from)
    }

    pub(crate) fn get_f64(&self, name: &str) -> Result<f64, MpvError> {
        self.inner.get_property(name).map_err(MpvError::from)
    }

    pub(crate) fn get_i64(&self, name: &str) -> Result<i64, MpvError> {
        self.inner.get_property(name).map_err(MpvError::from)
    }

    pub(crate) fn get_bool(&self, name: &str) -> Result<bool, MpvError> {
        self.inner.get_property(name).map_err(MpvError::from)
    }

    pub(crate) fn get_string(&self, name: &str) -> Result<String, MpvError> {
        self.inner.get_property(name).map_err(MpvError::from)
    }

    /// Sends a command and waits for mpv to accept it. A command that fails
    /// while it runs reports through events, not here.
    pub(crate) fn command(&self, name: &str, args: &[&str]) -> Result<(), MpvError> {
        self.inner.command(name, args).map_err(MpvError::from)
    }

    /// Queues `name` without waiting for its reply. The backend reports only
    /// a synchronous send failure here; command failures arrive as events.
    pub(crate) fn command_async(&self, name: &str, args: &[&str]) -> Result<(), MpvError> {
        let mut c_args: Vec<CString> = Vec::with_capacity(args.len() + 1);
        c_args.push(c_string(name)?);
        for arg in args {
            c_args.push(c_string(arg)?);
        }
        let mut ptrs: Vec<*const std::ffi::c_char> =
            c_args.iter().map(|arg| arg.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        // SAFETY: `self.inner` owns a live mpv context, and every pointer in
        // `ptrs` refers to a NUL-terminated string that stays alive for the
        // duration of this call.
        let rc = unsafe {
            libmpv2_sys::mpv_command_async(self.inner.ctx.as_ptr(), 0, ptrs.as_mut_ptr())
        };
        mpv_result(rc, "mpv_command_async")
    }

    /// Observes `name` for changes; each change arrives as a
    /// [`MpvEvent::PropertyChange`] carrying the value in `format`.
    pub(crate) fn observe_property(
        &self,
        name: &str,
        id: u64,
        format: Format,
    ) -> Result<(), MpvError> {
        self.inner
            .observe_property(name, format.into(), id)
            .map_err(MpvError::from)
    }

    /// Requests log messages at or above `level` (an mpv level name, for
    /// example `warn`), so they arrive as log-message events.
    pub(crate) fn request_log_messages(&self, level: &str) -> Result<(), MpvError> {
        let level = c_string(level)?;
        // SAFETY: `self.inner` owns a live mpv context, and `level` is a
        // NUL-terminated string that stays alive for the call.
        let rc = unsafe {
            libmpv2_sys::mpv_request_log_messages(self.inner.ctx.as_ptr(), level.as_ptr())
        };
        mpv_result(rc, "mpv_request_log_messages")
    }

    /// Pops the next event, or `None` when the backend has none now. The IPC
    /// session queues events that arrive while a request waits for its reply;
    /// the libmpv session polls the event queue.
    pub(crate) fn next_event(&self) -> Option<MpvEvent> {
        self.inner.wait_event(0.0).map(MpvEvent::from_libmpv)
    }

    /// The wakeup hook: the backend calls `callback` when events may be
    /// available. The run loop writes to its self-pipe with it.
    pub(crate) fn set_wakeup_callback(&mut self, callback: impl Fn() + Send + 'static) {
        self.inner.set_wakeup_callback(callback);
    }
}

/// Turns an mpv status code into the domain result.
fn mpv_result(rc: i32, call: &str) -> Result<(), MpvError> {
    if rc == 0 {
        return Ok(());
    }
    Err(MpvError::Libmpv(format!(
        "{call} failed: {} ({rc})",
        libmpv2_sys::mpv_error_str(rc)
    )))
}

fn c_string(value: &str) -> Result<CString, MpvError> {
    CString::new(value).map_err(|error| {
        MpvError::Libmpv(format!(
            "mpv argument contains a NUL byte at position {}: {value}",
            error.nul_position()
        ))
    })
}
