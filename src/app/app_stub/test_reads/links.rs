//! The two-Player-link fixture (`split-tui-gui-clients` row 2.3, correction
//! round 1), and the link-source facts a tick test stages or asserts
//! (`split-tui-gui-clients` design D4b rule 5, row 2.4): the active and
//! suspended Players, the Behavior endpoint that names the Owner a link
//! connects to, the direct-remote facts a routed peer leaves, the panel
//! targets a Sessions test offers, and the Playlists sidebar's fixture.
//!
//! The fixture exists for one contract: a reply that names a link must reach
//! that link's Player and no other. A single stub Player cannot tell the two
//! links apart, which is exactly how an inverted link mapping went unnoticed.

use super::super::App;
use mbv_ctrl::CtrlCmd;
use std::sync::mpsc;

impl App {
    /// Put a stub Player on the active connection that advertises `compat`,
    /// and answer the receiver of the commands it is sent. A test that scripts
    /// an Owner's advertised output capabilities and observes what a target
    /// activation sends takes this: the fixture names the compatibility the
    /// fabrication installs, not the fields that carry it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn set_player_with_compatibility_for_test(
        &mut self,
        compat: mbv_ctrl::CtrlCompatibility,
    ) -> mpsc::Receiver<CtrlCmd> {
        let (mut remote, player_rx, cmd_rx) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        remote.set_ctrl_compatibility_for_test(compat);
        self.player = mbv_player::PlayerProxy::from_remote(remote, false);
        self.player_rx = player_rx;
        cmd_rx
    }

    /// Offer `sessions` as the Sessions the last `/Sessions` load reported,
    /// and `receivers` as the cast receivers discovery found.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_sessions_and_receivers_for_test(
        &mut self,
        sessions: Vec<mbv_emby::SessionInfo>,
        receivers: Vec<mbv_cast::discovery::CastReceiver>,
    ) {
        self.sessions = sessions;
        self.cast_receivers = receivers;
    }

    /// Put a stub Player on the active connection, and keep what it receives.
    #[cfg(any(test, feature = "test"))]
    pub fn set_player_for_test(&mut self) {
        let (remote, player_rx, cmd_rx) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        remote.update_status(|status| status.volume_max = 100);
        self.player = mbv_player::PlayerProxy::from_remote(remote, false);
        self.player_rx = player_rx;
        ACTIVE_COMMANDS.with(|slot| *slot.borrow_mut() = Some(cmd_rx));
    }

    /// Hold `player` as the Player on the active connection: the stub a test
    /// built over its own advertised compatibility, whose command channel the
    /// stub recorded as that connection's.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_active_player_for_test(&mut self, player: mbv_player::PlayerProxy) {
        self.player = player;
    }

    /// Suspend a stub home link, and keep what it receives. The active
    /// connection keeps answering for the peer link beside it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_suspended_local_for_test(&mut self) {
        let (remote, player_rx, cmd_rx) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        self.suspended_local = Some(crate::app::state::playback::SuspendedLocalSession {
            player: mbv_player::PlayerProxy::from_remote(remote, false),
            player_rx,
        });
        HOME_COMMANDS.with(|slot| *slot.borrow_mut() = Some(cmd_rx));
    }

    /// Whether the active connection has received a `SwapPrepared` reply
    /// since the last take, draining what it received.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn took_swap_prepared_on_active_for_test(&mut self) -> bool {
        ACTIVE_COMMANDS.with(took_swap_prepared)
    }

    /// Whether the suspended home link has received a `SwapPrepared` reply
    /// since the last take, draining what it received.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn took_swap_prepared_on_home_for_test(&mut self) -> bool {
        HOME_COMMANDS.with(took_swap_prepared)
    }

    /// Suspend the home Player the `suspended_local` seam installed and
    /// deliver its Player events over `event_rx`, so a test drains one
    /// injected `PlayerEvent` inside a step: what the home link sends is the
    /// test's to send.
    ///
    /// # Panics
    ///
    /// Panics when no home link is suspended: the events would have no link
    /// to arrive on.
    #[cfg(any(test, feature = "test"))]
    pub fn set_suspended_local_events_for_test(
        &mut self,
        event_rx: mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
    ) {
        self.suspended_local
            .as_mut()
            .expect("a suspended home link to deliver events over")
            .player_rx = event_rx;
    }

    /// Attach a stub peer Owner exactly as the Sessions-panel's direct-remote
    /// connect does, so a test drives the disconnect and hand-off paths the
    /// production switch runs. `items` becomes the peer's queue and
    /// `peer_cmd_rx` answers what the peer is sent.
    #[cfg(any(test, feature = "test"))]
    pub fn switch_to_direct_remote_for_test(
        &mut self,
        device_name: &str,
        items: Vec<mbv_emby_model::EmbyItem>,
        endpoint: &mbv_remote_player::DaemonEndpoint,
    ) {
        let (remote, _) = mbv_remote_player::RemotePlayer::stub(items, 0);
        let sess = mbv_emby::test_support::make_session(device_name, "mbv");
        self.switch_to_direct_remote(&sess, remote, mpsc::channel().1, endpoint);
    }

    /// Attach a caller-built peer Owner exactly as the Sessions-panel's
    /// direct-remote connect does, so a test that holds the peer's command
    /// receiver (to assert the peer was sent nothing) installs that same
    /// connection. `device_name` is the peer's name, as the Session reports it.
    #[cfg(any(test, feature = "test"))]
    pub fn attach_direct_remote_for_test(
        &mut self,
        device_name: &str,
        remote: mbv_remote_player::RemotePlayer,
        peer_events: mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
        endpoint: &mbv_remote_player::DaemonEndpoint,
    ) {
        let sess = mbv_emby::test_support::make_session(device_name, "mbv");
        self.switch_to_direct_remote(&sess, remote, peer_events, endpoint);
    }

    /// Whether the home link is suspended behind a peer connection.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn home_link_is_suspended_for_test(&self) -> bool {
        self.suspended_local.is_some()
    }

    /// Whether a peer's queue is held beside the home queue: the state a
    /// direct remote-control connection installs.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn has_remote_view_for_test(&self) -> bool {
        self.remote_view.is_some()
    }

    /// Deliver the active link's Player events over `event_rx`, so a test
    /// drains one injected `PlayerEvent` inside a step.
    #[cfg(any(test, feature = "test"))]
    pub fn set_active_player_events_for_test(
        &mut self,
        event_rx: mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
    ) {
        self.player_rx = event_rx;
    }

    /// Hold `endpoint` as the endpoint this Client connects to: `Local` for a
    /// local-daemon Owner, `Tcp`/`Unix` for a routed peer.
    #[cfg(any(test, feature = "test"))]
    pub fn set_player_endpoint_for_test(&mut self, endpoint: mbv_remote_player::DaemonEndpoint) {
        self.player_endpoint = Some(endpoint);
    }

    /// The endpoint this Client connects to.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn player_endpoint_for_test(&self) -> Option<&mbv_remote_player::DaemonEndpoint> {
        self.player_endpoint.as_ref()
    }

    /// Whether a direct remote-control connection to a peer is held.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn direct_remote_connected_for_test(&self) -> bool {
        self.direct_remote_connected
    }

    /// The peer's name this Client labels its direct connection with.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn direct_remote_label_for_test(&self) -> Option<&str> {
        self.direct_remote_label.as_deref()
    }

    /// The Emby Session id the peer's direct connection was established over.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn direct_remote_session_id_for_test(&self) -> Option<&str> {
        self.direct_remote_session_id.as_deref()
    }

    /// Hold the routed-peer facts as a direct connect leaves them, then offer
    /// `target` as the one panel target the Sessions panel lists.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_direct_remote_for_test(
        &mut self,
        endpoint: mbv_remote_player::DaemonEndpoint,
        session_id: String,
        label: String,
    ) {
        self.player_endpoint = Some(endpoint);
        self.direct_remote_connected = true;
        self.direct_remote_session_id = Some(session_id);
        self.direct_remote_label = Some(label);
    }

    /// Offer `targets` as the panel targets the Sessions panel lists, as a
    /// target rebuild from a `/Sessions` load leaves them.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_panel_targets_for_test(
        &mut self,
        targets: Vec<crate::app::state::panel_targets::PanelTarget>,
    ) {
        self.panel_targets = targets;
    }

    /// Offer `playlists` as the Playlists tab's rows, each one a sidebar
    /// carrier row targeted by its own id, with the sidebar closed so a test
    /// observes the open its own action runs.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_playlists_for_test(&mut self, playlists: Vec<mbv_emby_model::EmbyItem>) {
        self.playlists = playlists;
        self.playlists_open = None;
        self.playlists_open_items.clear();
    }
}

/// Drain `slot`'s receiver and answer whether a `SwapPrepared` reply was among
/// what it held.
#[cfg(any(test, feature = "test"))]
fn took_swap_prepared(slot: &std::cell::RefCell<Option<mpsc::Receiver<CtrlCmd>>>) -> bool {
    let mut slot = slot.borrow_mut();
    let Some(receiver) = slot.as_mut() else {
        return false;
    };
    receiver
        .try_iter()
        .any(|command| matches!(command, CtrlCmd::SwapPrepared))
}

thread_local! {
    /// What the fixture's active stub Player received. Per thread, so two
    /// tests running side by side never read each other's commands.
    static ACTIVE_COMMANDS: std::cell::RefCell<Option<mpsc::Receiver<CtrlCmd>>> =
        const { std::cell::RefCell::new(None) };
    /// What the fixture's suspended home stub Player received.
    static HOME_COMMANDS: std::cell::RefCell<Option<mpsc::Receiver<CtrlCmd>>> =
        const { std::cell::RefCell::new(None) };
}
