//! The GUI Library's Client-only fetch channel (design D5 of
//! `decouple-client-core`): the workers that fetch a group's albums, a plain
//! Emby library's root list, and a Shows library's New episodes report here
//! instead of the shared library event channel.
//!
//! The channel lives on the `App`, so a worker that starts before the GUI's
//! core exists still has a sender to clone, and so does a worker that outlives
//! the core that started it. The receiver lives there too, behind a `Mutex`
//! because the drain takes `&self` and an `mpsc::Receiver` takes `&mut self`.
//! One `App` owns the one receiver, so no `Arc` shares it: every GUI Client
//! core over that `App` drains the one channel every worker sends to. A result
//! wakes the Client through the same `ClientWaker` the library channel uses,
//! so a landed fetch repaints without waiting out the idle cadence. The TUI
//! Client never starts one of these fetches, so its channel never holds an
//! event.

use std::sync::{Mutex, PoisonError, mpsc};

use crate::app::ClientWaker;
use mbv_emby_model::EmbyItem;

/// One Client-only fetch's completion, drained from the channel the GUI
/// Client's core owns. `pub` because a dependent Client's test scripts one
/// onto that channel, and because the core's channel carries it.
#[derive(Debug)]
pub enum GuiClientEvent {
    /// One group's album list landed.
    GroupAlbums {
        lib_idx: usize,
        group_id: String,
        result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
    },
    /// One plain Emby library's root list landed.
    EmbyRootItems {
        lib_idx: usize,
        result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
    },
    /// One TV library's latest episodes landed, under the library's id.
    ShowsNew {
        library_id: String,
        result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
    },
}

/// A worker-side sender that wakes the Client after each result it sends.
///
/// The wake runs after the send, so the tick a wake raises always finds the
/// result queued. The waker is a clone of the `App`'s shared handle, so it
/// reaches the callback the Client installed at start.
#[derive(Debug)]
pub struct GuiClientEventSender {
    tx: mpsc::Sender<GuiClientEvent>,
    waker: ClientWaker,
}

impl GuiClientEventSender {
    /// Send one result and wake the Client. A send with no live receiver is
    /// dropped: no Client is left to tell.
    pub fn send(&self, event: GuiClientEvent) {
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

/// The Client-only fetch channel: the worker senders, and the receiver the
/// `App` that owns it drains.
pub struct GuiClientEvents {
    sender: GuiClientEventSender,
    /// The receiver. The `App` holds the one copy, and every GUI Client core
    /// over that `App` drains this channel through it.
    rx: Mutex<mpsc::Receiver<GuiClientEvent>>,
}

impl GuiClientEvents {
    /// A fresh channel with no event in flight. A result wakes the Client
    /// through `waker`, as a library event does.
    pub fn new(waker: ClientWaker) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            sender: GuiClientEventSender { tx, waker },
            rx: Mutex::new(rx),
        }
    }

    /// The sender a worker clones.
    pub fn sender(&self) -> GuiClientEventSender {
        self.sender.clone()
    }

    /// Take one queued result, or `None` when none is queued.
    pub fn try_take(&self) -> Option<GuiClientEvent> {
        self.lock_receiver().try_recv().ok()
    }

    /// Wait up to `timeout` for one queued result, for a test that has no tick
    /// to wake it (task 8.3 of `decouple-client-core`). `None` when none
    /// arrives in time.
    #[cfg(any(test, feature = "test"))]
    pub(crate) fn wait_take_for_test(
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
