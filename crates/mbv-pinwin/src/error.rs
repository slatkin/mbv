//! The wrapper's failure type: one variant per documented pinwin result code.

use std::error::Error;
use std::fmt;

use crate::ffi;

/// A failure reported by the pinwin panel ABI.
#[derive(Debug)]
pub struct PinwinError(PinwinErrorKind);

#[derive(Debug)]
enum PinwinErrorKind {
    Invalid,
    AlreadyRunning,
    NotRunning,
    NoDisplay,
    Internal,
    Unknown(i32),
}

impl PinwinError {
    pub(crate) const fn from_code(code: i32) -> Self {
        let kind = match code {
            ffi::ERR_INVALID => PinwinErrorKind::Invalid,
            ffi::ERR_ALREADY_RUNNING => PinwinErrorKind::AlreadyRunning,
            ffi::ERR_NOT_RUNNING => PinwinErrorKind::NotRunning,
            ffi::ERR_NO_DISPLAY => PinwinErrorKind::NoDisplay,
            ffi::ERR_INTERNAL => PinwinErrorKind::Internal,
            other => PinwinErrorKind::Unknown(other),
        };
        Self(kind)
    }
}

impl fmt::Display for PinwinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            PinwinErrorKind::Invalid => f.write_str("pinwin refused the layout"),
            PinwinErrorKind::AlreadyRunning => f.write_str("a pinwin panel is already running"),
            PinwinErrorKind::NotRunning => f.write_str("no pinwin panel is running"),
            PinwinErrorKind::NoDisplay => {
                f.write_str("no Wayland layer-shell display is available")
            }
            PinwinErrorKind::Internal => f.write_str("pinwin reported an internal failure"),
            PinwinErrorKind::Unknown(code) => {
                write!(f, "pinwin returned unknown result code {code}")
            }
        }
    }
}

impl Error for PinwinError {}
