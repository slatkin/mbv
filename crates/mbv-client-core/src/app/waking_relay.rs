//! The wake relay (design D7 of `extract-client-core`): one thread per
//! waking channel forwards each upstream item in order, then calls the
//! installed `ClientWaker` so a sleeping Client repaints on the new input
//! instead of waiting out its idle cadence.

use std::sync::mpsc;
use std::thread;

use super::ClientWaker;

/// Relay `upstream` onto a fresh channel, waking the Client after each
/// forwarded item. The thread exits when the upstream sender drops, so a
/// closed channel still reads `Disconnected` at the drain, and when the
/// returned receiver drops, so a detached link leaks no thread.
pub(in crate::app) fn spawn_waking_relay<T: Send + 'static>(
    upstream: mpsc::Receiver<T>,
    waker: ClientWaker,
) -> mpsc::Receiver<T> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        while let Ok(item) = upstream.recv() {
            if tx.send(item).is_err() {
                return;
            }
            waker.wake();
        }
    });
    rx
}
