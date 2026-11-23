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

    /// The sender a GUI Library fetch worker clones (design D5 of
    /// `decouple-client-core`): the worker sends its result here instead of
    /// the shared library event channel, and the GUI Client's core drains it
    /// in `ClientTickHost::drain_client_events`. A TUI Client never starts one
    /// of these fetches.
    pub(in crate::app) fn gui_client_event_sender(
        &self,
    ) -> crate::app::state::gui_client_events::GuiClientEventSender {
        self.gui_client_events.sender()
    }

    /// Take one queued Client-only fetch result (design D5 of
    /// `decouple-client-core`), for the GUI's `ClientTickHost::drain_client_events`
    /// and for a test that drains over the `App` it holds.
    pub(crate) fn take_one_gui_client_event(
        &self,
    ) -> Option<crate::app::gui_library::client_events::GuiClientEvent> {
        self.gui_client_events.try_take()
    }

    /// Land one Client-only fetch result on this `App`'s channel, as a fetch
    /// worker would (design D5 of `decouple-client-core`): a dependent Client's
    /// test scripts the result instead of starting a worker, so the next tick's
    /// `drain_client_events` takes it from the same channel production fills.
    #[cfg(feature = "test")]
    #[doc(hidden)]
    pub fn send_gui_client_event_for_test(
        &self,
        event: crate::app::gui_library::client_events::GuiClientEvent,
    ) {
        self.gui_client_event_sender().send(event);
    }

    /// Wait up to `timeout` for one queued Client-only fetch result (task 8.3
    /// of `decouple-client-core`), for a test drain that has no tick to wake
    /// it.
    #[cfg(test)]
    pub(crate) fn wait_for_one_gui_client_event_for_test(
        &self,
        timeout: std::time::Duration,
    ) -> Option<crate::app::gui_library::client_events::GuiClientEvent> {
        self.gui_client_events.wait_take_for_test(timeout)
    }

    /// Record the `mbv` program this Client starts the Owner from (design
    /// D9 of `extract-client-core`). The Owner link sets it at attach time;
    /// `None` keeps this process's own executable.
    pub(crate) fn set_owner_exe(&mut self, owner_exe: Option<std::path::PathBuf>) {
        self.owner_exe = owner_exe;
    }

    /// Record the capabilities of this process's Client (design D6a of
    /// `decouple-client-core`). The Owner link sets them at attach time,
    /// before `App` exists, so the core reads them before the first step.
    pub(crate) fn set_client_capabilities(
        &mut self,
        capabilities: crate::app::state::client_capabilities::ClientCapabilities,
    ) {
        self.client_capabilities = capabilities;
    }

    /// Record a Client's capabilities from a dependent crate's tests
    /// (`decouple-client-core` design D6a): the GUI Client's headless tests
    /// drive an `App` without an Owner attach, so they set the set the
    /// attach would record.
    #[cfg(feature = "test")]
    #[doc(hidden)]
    pub fn set_client_capabilities_for_test(
        &mut self,
        capabilities: crate::app::state::client_capabilities::ClientCapabilities,
    ) {
        self.client_capabilities = capabilities;
    }

    /// Start this machine's Owner process from the Client's `mbv` program and
    /// wait until its control socket accepts connections. Every App-side
    /// Owner start goes through here so a GUI Client never starts itself as
    /// the Owner (design D9 of `extract-client-core`).
    pub(in crate::app) fn start_local_daemon(
        &self,
    ) -> Result<(), mbv_remote_player::RemotePlayerError> {
        crate::app::owner_link::start_owner(&crate::app::owner_link::OwnerStartOptions {
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
