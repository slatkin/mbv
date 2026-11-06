//! Toast presentation models (extract-client-core row 4.6): the neutral
//! severity class and the presented toast the Status bar's Surface state
//! carries. The TUI and GUI painters style the severity each in their own
//! way; the presented facts are the shared state.

use std::time::Duration;

/// Severity class for toast notifications. Neutral and Success display for
/// 2 s; Warning and Error display for 5 s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastSeverity {
    /// Progress, information, lifecycle events.
    #[default]
    Neutral,
    /// A user-requested action completed.
    Success,
    /// An operation failed but recovered through a working fallback.
    Warning,
    /// An operation failed without recovery.
    Error,
}

impl ToastSeverity {
    /// The severity-derived duration a toast displays
    /// (`toast-notification-semantics`).
    #[must_use]
    pub fn ttl(self) -> Duration {
        match self {
            ToastSeverity::Neutral | ToastSeverity::Success => Duration::from_secs(2),
            ToastSeverity::Warning | ToastSeverity::Error => Duration::from_secs(5),
        }
    }
}

/// The toast a live status row presents: its text and severity. Whether the
/// row presents it at all is the projecting Client's gating (desktop
/// notification delivery and expiry are shell concerns); the Surface state
/// carries the presented facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusToast {
    /// The toast's text.
    pub text: String,
    /// The toast's severity class, for the painters' severity colour.
    pub severity: ToastSeverity,
}
