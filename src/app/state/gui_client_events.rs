//! The GUI Library's Client-only fetch channel (design D5 of
//! `decouple-client-core`): the workers that fetch a group's albums, a plain
//! Emby library's root list, and a Shows library's New episodes report here
//! instead of the shared library event channel.
//!
//! The channel lives on the `App`, so a worker that starts before the GUI's
//! core exists still has a sender to clone. The receiver travels in an `Arc`
//! shared by every `GuiCore` built over that `App`, so a core parked beside a
//! moved-out `App` (the GUI's one-call `GuiCore` steps) still drains the one
//! channel the live core drains. A result wakes the Client through the same
//! `ClientWaker` the library channel uses, so a landed fetch repaints without
//! waiting out the idle cadence. The TUI Client never starts one of these
//! fetches, so its channel never holds an event.

use std::sync::{Arc, Mutex, PoisonError, mpsc};

use crate::app::ClientWaker;
use crate::app::gui_library::client_events::GuiClientEvent;

/// A worker-side sender that wakes the Client after each result it sends.
///
/// The wake runs after the send, so the tick a wake raises always finds the
/// result queued. The waker is a clone of the `App`'s shared handle, so it
/// reaches the callback the Client installed at start.
pub(in crate::app) struct GuiClientEventSender {
    tx: mpsc::Sender<GuiClientEvent>,
    waker: ClientWaker,
}

impl GuiClientEventSender {
    /// Send one result and wake the Client. A send with no live receiver is
    /// dropped: no Client is left to tell.
    pub(in crate::app) fn send(&self, event: GuiClientEvent) {
        if self.tx.send(event).is_ok() {
            self.waker.wake();
        }
    }
}

impl Clone for GuiClientEventSender {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            waker: self.waker.clone(),
        }
    }
}

/// The Client-only fetch channel: the worker senders, and the receiver every
/// `GuiCore` over this `App` drains.
pub(in crate::app) struct GuiClientEvents {
    sender: GuiClientEventSender,
    /// The shared receiver. Every `GuiCore` drains this one channel through
    /// the `App`, including a core built for one call beside a parked `App`.
    rx: Arc<Mutex<mpsc::Receiver<GuiClientEvent>>>,
}

impl GuiClientEvents {
    /// A fresh channel with no event in flight. A result wakes the Client
    /// through `waker`, as a library event does.
    pub(in crate::app) fn new(waker: ClientWaker) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            sender: GuiClientEventSender { tx, waker },
            rx: Arc::new(Mutex::new(rx)),
        }
    }

    /// The sender a worker clones.
    pub(in crate::app) fn sender(&self) -> GuiClientEventSender {
        self.sender.clone()
    }

    /// Take one queued result, or `None` when none is queued.
    pub(in crate::app) fn try_take(&self) -> Option<GuiClientEvent> {
        self.lock_receiver().try_recv().ok()
    }

    /// Wait up to `timeout` for one queued result, for a test that has no tick
    /// to wake it (task 8.3 of `decouple-client-core`). `None` when none
    /// arrives in time.
    #[cfg(test)]
    pub(in crate::app) fn wait_take_for_test(
        &self,
        timeout: std::time::Duration,
    ) -> Option<GuiClientEvent> {
        self.lock_receiver().recv_timeout(timeout).ok()
    }

    /// The receiver, recovered from a poisoned lock: a poisoned channel must
    /// never silence a landed fetch.
    fn lock_receiver(&self) -> std::sync::MutexGuard<'_, mpsc::Receiver<GuiClientEvent>> {
        self.rx.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
