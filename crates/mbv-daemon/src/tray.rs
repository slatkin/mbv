//! Tray lifecycle owned by the daemon runtime.
//!
//! `mbv-desktop` owns the ksni tray; the daemon only holds this port and
//! pushes the pinwin panel socket of the most recently declared pinned
//! Client (design D4). The trait lives here so the tray never reads the ctrl
//! registry.

use std::path::PathBuf;
use std::sync::mpsc;

/// Handle to a started tray. `mbv-desktop` implements it over its ksni
/// handle; the daemon only pushes the current pin target.
pub trait TrayPort: Send {
    /// Replace (or clear) the pinwin panel socket offered by the tray's pin
    /// item.
    fn set_pin_target(&self, socket: Option<PathBuf>);
}

/// The runtime tray hook. It starts the tray and returns its port, or `None`
/// when the tray icon is disabled or no desktop session is available.
pub(crate) type OnTrayReady = Box<dyn FnOnce(mpsc::SyncSender<()>) -> Option<Box<dyn TrayPort>>>;

/// Daemon-side tray state: an unstarted hook (stay-alive off), a started
/// port, or neither when the tray icon is disabled.
pub(crate) struct TraySlot {
    on_ready: Option<OnTrayReady>,
    port: Option<Box<dyn TrayPort>>,
    shutdown_signal_tx: mpsc::SyncSender<()>,
}

impl TraySlot {
    pub(crate) fn new(on_ready: OnTrayReady, shutdown_signal_tx: mpsc::SyncSender<()>) -> Self {
        Self {
            on_ready: Some(on_ready),
            port: None,
            shutdown_signal_tx,
        }
    }

    /// Invoke the one-shot hook. Taking it means a disabled tray icon is
    /// never retried, so the tray starts at most once per daemon.
    pub(crate) fn start(&mut self) {
        if let Some(on_ready) = self.on_ready.take() {
            self.port = on_ready(self.shutdown_signal_tx.clone());
        }
    }

    /// An accepted pinned declaration: start lazily (at most once) once there
    /// is a target to offer, then push the current target.
    pub(crate) fn on_declaration(&mut self, target: Option<PathBuf>) {
        if target.is_some() && self.port.is_none() {
            self.start();
        }
        self.set_pin_target(target);
    }

    /// Push to an already-started tray. A client removal must not start one.
    pub(crate) fn set_pin_target(&self, target: Option<PathBuf>) {
        if let Some(port) = &self.port {
            port.set_pin_target(target);
        }
    }
}
