//! The `App`'s Client-waker and Owner-start handles (design D7 and D9 of
//! `extract-client-core`): the shared wake handle every Owner-snapshot relay
//! carries, the `mbv` program the Client starts its Owner from, and the
//! waking relay an attached event channel is wrapped in.
//!
//! The group is cohesive — one waker value and the two places it decides who
//! starts the Owner and how an event reaches the drain — so it sits in its own
//! child module of `app_struct` and keeps that file within its line ceiling.

use mbv_ctrl::player::PlayerEvent;
use std::sync::mpsc;

use super::App;

impl App {
    /// The shared wake handle this `App` hands to its Owner-snapshot relays
    /// (design D7 of `extract-client-core`). The Client core shares it, so the
    /// waker the Client installs reaches every relay, including one the
    /// initial attach already spawned.
    #[must_use]
    pub fn client_waker(&self) -> crate::app::ClientWaker {
        self.client_waker.clone()
    }

    /// Record the `mbv` program this Client starts the Owner from (design
    /// D9 of `extract-client-core`). The Owner link sets it at attach time;
    /// `None` keeps this process's own executable.
    pub(crate) fn set_owner_exe(&mut self, owner_exe: Option<std::path::PathBuf>) {
        self.owner_exe = owner_exe;
    }

    /// Start this machine's Owner process from the Client's `mbv` program and
    /// wait until its control socket accepts connections. Every App-side
    /// Owner start goes through here so a GUI Client never starts itself as
    /// the Owner (design D9 of `extract-client-core`).
    pub(in crate::app) fn start_local_daemon(
        &self,
    ) -> Result<(), mbv_remote_player::RemotePlayerError> {
        crate::owner_link::start_owner(&crate::owner_link::OwnerStartOptions {
            owner_exe: self.owner_exe.as_deref(),
            log_level: None,
        })
    }

    /// Relay one freshly attached Owner event channel through the wake
    /// relay (design D7 of `extract-client-core`): forwarded events reach the
    /// drain in order and each send wakes the Client. Receivers already
    /// behind a relay — a resumed suspended home link included — must not
    /// be re-wrapped.
    pub(in crate::app) fn waking_player_rx(
        &self,
        rx: mpsc::Receiver<PlayerEvent>,
    ) -> mpsc::Receiver<PlayerEvent> {
        crate::app::spawn_waking_relay(rx, self.client_waker.clone())
    }
}
