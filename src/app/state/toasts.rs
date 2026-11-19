//! The Client core's live toasts (`gui-floating-toasts` design D1). One list
//! holds them, so several toasts are live at once and the cap, the repeat rule,
//! and the per-toast severity-derived expiry live in one place. Every method
//! that depends on time takes `now`, so tests need no clock and no sleep.

use std::time::Instant;

use mbv_ui_model::toast::{ToastId, ToastSeverity};

/// At most this many toasts are live at once; a further toast ends the oldest
/// (`toast-notification-semantics`: a sixth toast ends the oldest of the five).
pub(in crate::app) const MAX_LIVE_TOASTS: usize = 5;

/// One live toast: its stable id, the presented facts, and when it ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct LiveToast {
    pub(in crate::app) id: ToastId,
    pub(in crate::app) text: String,
    pub(in crate::app) severity: ToastSeverity,
    pub(in crate::app) expires: Instant,
}

/// The Client core's live toasts, oldest first.
#[derive(Debug, Default)]
pub(in crate::app) struct ToastList {
    entries: Vec<LiveToast>,
    next_id: u64,
}

impl ToastList {
    /// Appends a toast and gives it a fresh id and a full severity-derived
    /// duration. Expired entries go first, then a live entry with the same text
    /// and severity (the repeat rule), then the oldest entry when the list is
    /// full.
    pub(in crate::app) fn push(&mut self, text: String, severity: ToastSeverity, now: Instant) {
        self.expire(now);
        self.entries
            .retain(|entry| entry.text != text || entry.severity != severity);
        if self.entries.len() >= MAX_LIVE_TOASTS {
            self.entries.remove(0);
        }
        let id = ToastId(self.next_id);
        self.next_id += 1;
        self.entries.push(LiveToast {
            id,
            text,
            severity,
            expires: now + severity.ttl(),
        });
    }

    /// Every toast still live at `now`, oldest first.
    pub(in crate::app) fn live(&self, now: Instant) -> impl Iterator<Item = &LiveToast> {
        self.entries.iter().filter(move |entry| entry.expires > now)
    }

    /// Drops the toasts whose duration has run out at `now`.
    pub(in crate::app) fn expire(&mut self, now: Instant) {
        self.entries.retain(|entry| entry.expires > now);
    }

    /// Drops every toast.
    pub(in crate::app) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests;
