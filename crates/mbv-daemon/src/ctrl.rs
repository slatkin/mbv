//! Ctrl transport registry shared by the daemon event-loop modules.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mbv_ctrl::{
    AttachedClients, AttachedClientsCell, CtrlAudiobookshelfCapabilities, CtrlEvent,
    DisconnectReason,
};

pub(crate) type CtrlClientId = u64;
pub(crate) type CtrlSender = mpsc::Sender<CtrlOutbound>;

pub(crate) enum CtrlOutbound {
    Event(String),
    /// Writer-thread barrier used during daemon shutdown. The ack is sent
    /// only after all earlier events have been written and flushed to the
    /// socket, so process exit cannot discard a queued shutdown notice.
    Flush(mpsc::Sender<()>),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum AuthorityHolder {
    #[default]
    None,
    Ctrl,
    EmbyRemote,
}

#[derive(Default)]
pub(crate) struct CtrlClients {
    next_id: CtrlClientId,
    connection: Vec<CtrlClient>,
    held_client: bool,
    merged_tx: Option<mpsc::Sender<crate::DaemonEvent>>,
    pub(crate) shutting_down: bool,
    pub(crate) authority: AuthorityHolder,
    /// Shared with the [`DaemonPlayerHandle`](crate::DaemonPlayerHandle) so
    /// the Tray can read which local Clients are attached without touching
    /// the registry. Refreshed on connect and disconnect (design D1).
    attached_clients: Arc<AttachedClientsCell>,
    /// The one-shot admission token of a Pin swap waiting for its
    /// replacement (tray-pin-swap design D4), shared with the
    /// [`PinSwapState`](crate::event_loop::PinSwapState) machine, which
    /// publishes and clears it; the admission path consumes it.
    pending_swap: Arc<crate::PendingSwapToken>,
}

/// Which role a ctrl connection plays, classified from its Hello
/// (tray-pin-swap design D7). `Client` is a full TUI; the two admin roles
/// are restricted single-command connections that keep the admin admission
/// and registry rules — never a driver, never held, never a Pin target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CtrlConnectionRole {
    Client,
    ServiceSetupAdmin,
    OwnerAction,
}

/// Transport identity for a ctrl client connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtrlTransport {
    /// Local Unix socket listener.
    Local,
    /// TCP listener.
    Tcp,
}

/// Which swap-capable surface a ctrl client runs on, derived at Hello from
/// the `pin-swap` and `pinned-surface` capabilities (tray-pin-swap design
/// D2, as amended: a pinned Client is a Pinned target even without
/// `pin-swap`). Only a Terminal target requires `pin-swap`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SwapSurface {
    Terminal,
    Pinned,
}

struct CtrlClient {
    id: CtrlClientId,
    tx: CtrlSender,
    transport: CtrlTransport,
    /// Capabilities advertised by this peer at Hello.
    audiobookshelf: CtrlAudiobookshelfCapabilities,
    supports_owner_queue_load: bool,
    /// Whether the peer negotiated `playback-output` at Hello
    /// (cast-owner-output design D3); gates `CtrlEvent::PlaybackOutput`.
    supports_playback_output: bool,
    role: CtrlConnectionRole,
    swap_surface: Option<SwapSurface>,
}

pub(crate) type ClientRegistry = Arc<Mutex<CtrlClients>>;

/// Send an event to a single ctrl-socket client, rather than every connected
/// TUI. Used for per-request responses like a command rejection (#90).
pub(crate) fn send_to(client: &CtrlSender, event: &CtrlEvent) {
    if let Some(json) = serialize_ctrl_event(event) {
        let _ = client.send(CtrlOutbound::Event(json));
    }
}

/// Shared by `broadcast` and `send_to` so both go through one serialization
/// path instead of repeating `serde_json::to_string(event).ok()` inline.
pub(crate) fn serialize_ctrl_event(event: &CtrlEvent) -> Option<String> {
    serde_json::to_string(event).ok()
}

impl CtrlClients {
    pub(crate) fn new(
        merged_tx: mpsc::Sender<crate::DaemonEvent>,
        attached_clients: Arc<AttachedClientsCell>,
        pending_swap: Arc<crate::PendingSwapToken>,
    ) -> Self {
        Self {
            merged_tx: Some(merged_tx),
            attached_clients,
            pending_swap,
            ..Self::default()
        }
    }

    /// The pending Pin-swap admission token (design D4). The registry only
    /// carries it: the machine publishes and clears it, the admission path
    /// consumes it.
    pub(crate) fn pending_swap(&self) -> &crate::PendingSwapToken {
        &self.pending_swap
    }

    fn notify_last_client_gone(&self, was_nonempty: bool) {
        if was_nonempty
            && self.connection.is_empty()
            && self.held_client
            && let Some(merged_tx) = &self.merged_tx
        {
            let _ = merged_tx.send(crate::DaemonEvent::LastClientGone);
        }
    }

    fn retain_clients(&mut self, keep: impl FnMut(&CtrlClient) -> bool) {
        let before = self.connection.len();
        self.connection.retain(keep);
        self.notify_last_client_gone(before > 0);
        if self.connection.len() != before {
            self.refresh_attached_clients();
        }
    }

    /// Append `tx` as a new ctrl connection. Multiple clients may coexist.
    /// Does NOT override authority if it is currently `EmbyRemote` — the new
    /// client receives broadcasts but its commands are rejected until
    /// authority returns to `Ctrl`. A non-Client role keeps the admin
    /// registry rules (design D7): never a driver, never held, never a Pin
    /// target.
    pub(crate) fn connect_with_role(
        &mut self,
        tx: CtrlSender,
        transport: CtrlTransport,
        audiobookshelf: CtrlAudiobookshelfCapabilities,
        supports_owner_queue_load: bool,
        supports_playback_output: bool,
        swap_surface: Option<SwapSurface>,
        role: CtrlConnectionRole,
    ) -> CtrlClientId {
        let id = self.next_id;
        self.next_id += 1;
        self.held_client |= role == CtrlConnectionRole::Client;
        // A non-Client role is never a Pin target (design D7), whatever its
        // Hello advertised.
        let swap_surface = match role {
            CtrlConnectionRole::Client => swap_surface,
            CtrlConnectionRole::ServiceSetupAdmin | CtrlConnectionRole::OwnerAction => None,
        };
        self.connection.push(CtrlClient {
            id,
            tx,
            transport,
            audiobookshelf,
            supports_owner_queue_load,
            supports_playback_output,
            role,
            swap_surface,
        });
        if role == CtrlConnectionRole::Client && self.authority == AuthorityHolder::None {
            self.authority = AuthorityHolder::Ctrl;
        }
        self.refresh_attached_clients();
        id
    }

    /// Recomputes the shared attached-Clients state from the current
    /// connection set (design D1): `Pinned` when a local Client runs the
    /// pinned surface, otherwise `Unpinned` when at least one local
    /// connection has the `Client` role (whether or not it advertises
    /// `pin-swap`), otherwise `None`. Owner-action and admin roles and TCP
    /// connections never count. Called on connect and on every disconnect
    /// path (`remove` and dropped-peer pruning both funnel through
    /// `retain_clients`).
    fn refresh_attached_clients(&mut self) {
        let attached = if self.newest_swap_client(SwapSurface::Pinned).is_some() {
            AttachedClients::Pinned
        } else if self.connection.iter().any(|client| {
            client.role == CtrlConnectionRole::Client && client.transport == CtrlTransport::Local
        }) {
            AttachedClients::Unpinned
        } else {
            AttachedClients::None
        };
        self.attached_clients.store(attached);
    }

    /// The newest local-transport Client whose swap surface is `surface`
    /// (design D3): with [`SwapSurface::Pinned`] this is the pinned Client
    /// the Unpin direction replaces; with [`SwapSurface::Terminal`] it is
    /// the last-connected unpinned Client the Pin direction replaces (spec
    /// pin-swap "Pin replaces the newest unpinned Client"). Only
    /// local-transport Clients count (spec pin-swap "The Tray offers one Pin
    /// swap action").
    pub(crate) fn newest_swap_client(&self, surface: SwapSurface) -> Option<CtrlClientId> {
        self.connection
            .iter()
            .rev()
            .find(|client| {
                client.transport == CtrlTransport::Local && client.swap_surface == Some(surface)
            })
            .map(|client| client.id)
    }

    pub(crate) fn remove(&mut self, id: CtrlClientId) {
        self.retain_clients(|client| client.id != id);
        if self.connection.is_empty() && self.authority == AuthorityHolder::Ctrl {
            self.authority = AuthorityHolder::None;
        }
    }

    pub(crate) fn has_client(&self, id: CtrlClientId) -> bool {
        self.connection.iter().any(|c| c.id == id)
    }

    pub(crate) fn is_local_client(&self, id: CtrlClientId) -> bool {
        self.connection
            .iter()
            .any(|c| c.id == id && c.transport == CtrlTransport::Local)
    }

    pub(crate) fn transport(&self, id: CtrlClientId) -> Option<CtrlTransport> {
        self.connection
            .iter()
            .find(|client| client.id == id)
            .map(|client| client.transport)
    }

    /// Whether the client `id` advertised `abs-queue` support at Hello.
    /// Used to gate Audiobookshelf `QueueItem` transport in both directions.
    pub(crate) fn supports_abs_queue(&self, id: CtrlClientId) -> bool {
        self.connection
            .iter()
            .find(|c| c.id == id)
            .is_some_and(|c| c.audiobookshelf.queue)
    }

    /// Whether the client `id` advertised `abs-book-queue` support at Hello.
    /// Used to gate Audiobookshelf book `QueueItem` transport in both
    /// directions.
    pub(crate) fn supports_owner_queue_load(&self, id: CtrlClientId) -> bool {
        self.connection
            .iter()
            .find(|c| c.id == id)
            .is_some_and(|c| c.supports_owner_queue_load)
    }

    pub(crate) fn supports_abs_book_queue(&self, id: CtrlClientId) -> bool {
        self.connection
            .iter()
            .find(|c| c.id == id)
            .is_some_and(|c| c.audiobookshelf.book_queue)
    }

    pub(crate) fn send_to_client(&self, id: CtrlClientId, event: &CtrlEvent) {
        if let Some(client) = self.connection.iter().find(|client| client.id == id) {
            send_to(&client.tx, event);
        }
    }

    pub(crate) fn has_driver(&self) -> bool {
        self.connection
            .iter()
            .any(|client| client.role == CtrlConnectionRole::Client)
    }

    /// Broadcast `json` to all connected ctrl clients. Removes any client
    /// whose channel has failed (broken pipe / disconnected).
    pub(crate) fn broadcast_to_all(&mut self, json: &str) {
        self.retain_clients(|client| {
            client
                .tx
                .send(CtrlOutbound::Event(json.to_string()))
                .is_ok()
        });
    }

    /// Broadcasts a state event gated per client, excluding `except` when
    /// provided:
    /// - `unified_full_json` → peers advertising `abs-queue` + `abs-book-queue`
    /// - `unified_abs_json` → peers advertising `abs-queue` only
    /// - `unified_book_json` → peers advertising `abs-book-queue` only
    /// - `unified_json` → all others
    ///
    /// Mirrors `broadcast_to_all`'s drop-on-failed-send behavior.
    pub(crate) fn broadcast_state_gated(
        &mut self,
        unified_full_json: &str,
        unified_abs_json: &str,
        unified_book_json: &str,
        unified_json: &str,
        except: Option<CtrlClientId>,
    ) {
        self.retain_clients(|client| {
            if except == Some(client.id) {
                return true;
            }
            let json = match (
                client.audiobookshelf.queue,
                client.audiobookshelf.book_queue,
            ) {
                (true, true) => unified_full_json,
                (true, false) => unified_abs_json,
                (false, true) => unified_book_json,
                (false, false) => unified_json,
            };
            client
                .tx
                .send(CtrlOutbound::Event(json.to_string()))
                .is_ok()
        });
    }

    /// Sends redacted Audiobookshelf progress `json` only to clients that
    /// advertised `abs-progress` at Hello. Peers lacking the capability are
    /// silently skipped (not dropped) — mirrors `broadcast_state_gated`'s
    /// drop-on-failed-send semantics for the peers that do receive it.
    pub(crate) fn broadcast_progress_gated(&mut self, json: &str) {
        self.retain_clients(|client| {
            if !client.audiobookshelf.progress {
                return true;
            }
            client
                .tx
                .send(CtrlOutbound::Event(json.to_string()))
                .is_ok()
        });
    }

    /// Sends redacted Audiobookshelf book progress `json` only to clients
    /// that advertised `abs-book-progress` at Hello.
    pub(crate) fn broadcast_book_progress_gated(&mut self, json: &str) {
        self.retain_clients(|client| {
            if !client.audiobookshelf.book_progress {
                return true;
            }
            client
                .tx
                .send(CtrlOutbound::Event(json.to_string()))
                .is_ok()
        });
    }

    /// Sends the playback-output event `json` only to clients that negotiated
    /// `playback-output` at Hello (cast-owner-output design D3). Peers
    /// lacking the capability are silently skipped, mirroring
    /// `broadcast_progress_gated`.
    pub(crate) fn broadcast_playback_output_gated(&mut self, json: &str) {
        self.retain_clients(|client| {
            if !client.supports_playback_output {
                return true;
            }
            client
                .tx
                .send(CtrlOutbound::Event(json.to_string()))
                .is_ok()
        });
    }

    /// Broadcast a `Disconnected` notification to all connected ctrl clients
    /// without closing their connections. Used for Emby remote authority
    /// transitions — clients observe the authority change but stay connected.
    pub(crate) fn notify_disconnected_all(&self, reason: DisconnectReason) {
        for client in &self.connection {
            send_to(&client.tx, &CtrlEvent::Disconnected { reason });
        }
    }

    /// Wait until every currently connected writer has drained the events
    /// queued before its barrier. A single deadline bounds the whole client
    /// set; a broken or non-reading socket must not hold daemon shutdown
    /// forever.
    pub(crate) fn flush_writers(&self, timeout: Duration) {
        let acks: Vec<_> = self
            .connection
            .iter()
            .filter_map(|client| {
                let (ack_tx, ack_rx) = mpsc::channel();
                client
                    .tx
                    .send(CtrlOutbound::Flush(ack_tx))
                    .ok()
                    .map(|()| ack_rx)
            })
            .collect();
        let deadline = Instant::now() + timeout;
        for ack_rx in acks {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let _ = ack_rx.recv_timeout(remaining);
        }
    }

    pub(crate) fn begin_shutdown(&mut self) {
        self.shutting_down = true;
    }

    pub(crate) fn take_authority_for_emby_remote(&mut self) {
        self.notify_disconnected_all(DisconnectReason::TakenOverByEmbyRemote);
        self.authority = AuthorityHolder::EmbyRemote;
    }
}

pub(crate) fn take_authority_for_emby_remote(ctrl_clients: &ClientRegistry) {
    ctrl_clients
        .lock()
        .unwrap()
        .take_authority_for_emby_remote();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clients() -> CtrlClients {
        let (tx, _rx) = mpsc::channel();
        CtrlClients::new(
            tx,
            Arc::new(AttachedClientsCell::default()),
            Arc::new(crate::PendingSwapToken::default()),
        )
    }

    fn connect_surface(
        clients: &mut CtrlClients,
        transport: CtrlTransport,
        surface: Option<SwapSurface>,
    ) -> CtrlClientId {
        let (tx, _rx) = mpsc::channel();
        clients.connect_with_role(
            tx,
            transport,
            CtrlAudiobookshelfCapabilities::default(),
            false,
            false,
            surface,
            CtrlConnectionRole::Client,
        )
    }

    /// Contract: spec pin-swap "Pin replaces the newest unpinned Client",
    /// two-Client scenario — the Pin target is the terminal Client that
    /// attached last, and falls back to the older one when it leaves.
    #[test]
    fn pin_targets_the_newest_attached_terminal_client() {
        let mut clients = clients();
        let first = connect_surface(
            &mut clients,
            CtrlTransport::Local,
            Some(SwapSurface::Terminal),
        );
        let second = connect_surface(
            &mut clients,
            CtrlTransport::Local,
            Some(SwapSurface::Terminal),
        );

        assert_eq!(
            clients.newest_swap_client(SwapSurface::Terminal),
            Some(second)
        );

        clients.remove(second);

        assert_eq!(
            clients.newest_swap_client(SwapSurface::Terminal),
            Some(first)
        );
    }

    /// Contract: spec pin-swap "The Tray offers one Pin swap action" — only
    /// Clients attached over the local transport count as swap targets, and
    /// a TCP pinned Client never sets the shared flag.
    #[test]
    fn tcp_clients_are_never_swap_clients() {
        let mut clients = clients();
        connect_surface(
            &mut clients,
            CtrlTransport::Tcp,
            Some(SwapSurface::Terminal),
        );
        connect_surface(&mut clients, CtrlTransport::Tcp, Some(SwapSurface::Pinned));

        assert_eq!(clients.newest_swap_client(SwapSurface::Terminal), None);
        assert_eq!(clients.newest_swap_client(SwapSurface::Pinned), None);
        // Two TCP Clients never count as attached local Clients (design D1).
        assert_eq!(clients.attached_clients.load(), AttachedClients::None);
    }

    /// Contract: spec owner-actions "An Owner action request is not a
    /// Client" / design D7 — the two admin roles keep the admin registry
    /// rules: never a driver, never the authority holder.
    #[test]
    fn non_client_roles_are_never_drivers_or_authority_holders() {
        let mut clients = clients();
        let (tx, _rx) = mpsc::channel();
        clients.connect_with_role(
            tx,
            CtrlTransport::Local,
            CtrlAudiobookshelfCapabilities::default(),
            false,
            false,
            None,
            CtrlConnectionRole::OwnerAction,
        );
        let (tx, _rx) = mpsc::channel();
        clients.connect_with_role(
            tx,
            CtrlTransport::Local,
            CtrlAudiobookshelfCapabilities::default(),
            false,
            false,
            None,
            CtrlConnectionRole::ServiceSetupAdmin,
        );

        assert!(!clients.has_driver());
        assert_eq!(clients.authority, AuthorityHolder::None);
    }

    /// Contract: spec pin-swap "The Tray offers Open while no Client is
    /// attached" — the shared state is `Pinned` while a pinned Client is
    /// attached and `None` when it leaves (the Open item shows only for
    /// `None`).
    #[test]
    fn pinned_state_clears_when_the_pinned_client_leaves() {
        let mut clients = clients();
        let pinned = connect_surface(
            &mut clients,
            CtrlTransport::Local,
            Some(SwapSurface::Pinned),
        );

        assert_eq!(clients.attached_clients.load(), AttachedClients::Pinned);

        clients.remove(pinned);

        assert_eq!(clients.attached_clients.load(), AttachedClients::None);
    }

    /// Contract: spec pin-swap "The Tray offers Open while no Client is
    /// attached" — a local Client with no pinned surface reports `Unpinned`
    /// (whether or not it advertises `pin-swap`), while an Owner-action-role
    /// connection alone reports `None`.
    #[test]
    fn unpinned_local_clients_report_unpinned_and_owner_action_roles_report_none() {
        let mut registry = clients();
        connect_surface(&mut registry, CtrlTransport::Local, None);
        assert_eq!(registry.attached_clients.load(), AttachedClients::Unpinned);

        let mut admin_only = clients();
        let (tx, _rx) = mpsc::channel();
        admin_only.connect_with_role(
            tx,
            CtrlTransport::Local,
            CtrlAudiobookshelfCapabilities::default(),
            false,
            false,
            None,
            CtrlConnectionRole::OwnerAction,
        );
        assert_eq!(admin_only.attached_clients.load(), AttachedClients::None);
    }
}
