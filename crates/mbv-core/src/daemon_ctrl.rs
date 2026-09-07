//! Ctrl transport registry shared by the daemon event-loop modules.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::ctrl::{CtrlEvent, DisconnectReason};

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
    pub(crate) authority: AuthorityHolder,
}

/// Transport identity for a ctrl client connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtrlTransport {
    /// Local Unix socket listener.
    Local,
    /// TCP listener.
    Tcp,
}

struct CtrlClient {
    id: CtrlClientId,
    tx: CtrlSender,
    transport: CtrlTransport,
    /// Whether this peer advertised `abs-queue` in its Hello. Gates whether
    /// it receives or may submit `QueueItem::Audiobookshelf` values.
    supports_abs_queue: bool,
    /// Whether this peer advertised `abs-progress` in its Hello. Gates
    /// whether it receives the redacted Audiobookshelf progress event.
    supports_abs_progress: bool,
    /// Whether this peer advertised `abs-book-queue` in its Hello. Gates
    /// whether it receives or may submit `QueueItem::AudiobookshelfBook`.
    supports_abs_book_queue: bool,
    /// Whether this peer advertised `abs-book-progress` in its Hello. Gates
    /// whether it receives the redacted Audiobookshelf book progress event.
    supports_abs_book_progress: bool,
}

pub(crate) type ClientRegistry = Arc<Mutex<CtrlClients>>;

pub(crate) struct CtrlRequest<'a> {
    pub(crate) reply_tx: &'a CtrlSender,
}

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
    /// Append `tx` as a new ctrl connection. Multiple clients may coexist.
    /// Does NOT override authority if it is currently `EmbyRemote` — the new
    /// client receives broadcasts but its commands are rejected until
    /// authority returns to `Ctrl`.
    pub(crate) fn connect(
        &mut self,
        tx: CtrlSender,
        transport: CtrlTransport,
        supports_abs_queue: bool,
        supports_abs_progress: bool,
        supports_abs_book_queue: bool,
        supports_abs_book_progress: bool,
    ) -> CtrlClientId {
        let id = self.next_id;
        self.next_id += 1;
        self.connection.push(CtrlClient {
            id,
            tx,
            transport,
            supports_abs_queue,
            supports_abs_progress,
            supports_abs_book_queue,
            supports_abs_book_progress,
        });
        if self.authority == AuthorityHolder::None {
            self.authority = AuthorityHolder::Ctrl;
        }
        id
    }

    pub(crate) fn remove(&mut self, id: CtrlClientId) {
        self.connection.retain(|c| c.id != id);
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
            .is_some_and(|c| c.supports_abs_queue)
    }

    /// Whether the client `id` advertised `abs-book-queue` support at Hello.
    /// Used to gate Audiobookshelf book `QueueItem` transport in both
    /// directions.
    pub(crate) fn supports_abs_book_queue(&self, id: CtrlClientId) -> bool {
        self.connection
            .iter()
            .find(|c| c.id == id)
            .is_some_and(|c| c.supports_abs_book_queue)
    }

    pub(crate) fn send_to_client(&self, id: CtrlClientId, event: &CtrlEvent) {
        if let Some(client) = self.connection.iter().find(|client| client.id == id) {
            send_to(&client.tx, event);
        }
    }

    pub(crate) fn has_driver(&self) -> bool {
        !self.connection.is_empty()
    }

    /// Broadcast `json` to all connected ctrl clients. Removes any client
    /// whose channel has failed (broken pipe / disconnected).
    pub(crate) fn broadcast_to_all(&mut self, json: String) {
        self.connection
            .retain(|c| c.tx.send(CtrlOutbound::Event(json.clone())).is_ok());
    }

    /// Broadcasts a state event gated per client:
    /// - `unified_full_json` → peers advertising `abs-queue` + `abs-book-queue`
    /// - `unified_abs_json` → peers advertising `abs-queue` only
    /// - `unified_book_json` → peers advertising `abs-book-queue` only
    /// - `unified_json` → all others
    ///
    /// Mirrors `broadcast_to_all`'s drop-on-failed-send behavior.
    pub(crate) fn broadcast_state_gated(
        &mut self,
        unified_full_json: String,
        unified_abs_json: String,
        unified_book_json: String,
        unified_json: String,
    ) {
        self.connection.retain(|c| {
            let json = match (c.supports_abs_queue, c.supports_abs_book_queue) {
                (true, true) => &unified_full_json,
                (true, false) => &unified_abs_json,
                (false, true) => &unified_book_json,
                (false, false) => &unified_json,
            };
            c.tx.send(CtrlOutbound::Event(json.clone())).is_ok()
        });
    }

    /// Sends redacted Audiobookshelf progress `json` only to clients that
    /// advertised `abs-progress` at Hello. Peers lacking the capability are
    /// silently skipped (not dropped) — mirrors `broadcast_state_gated`'s
    /// drop-on-failed-send semantics for the peers that do receive it.
    pub(crate) fn broadcast_progress_gated(&mut self, json: String) {
        self.connection.retain(|c| {
            if !c.supports_abs_progress {
                return true;
            }
            c.tx.send(CtrlOutbound::Event(json.clone())).is_ok()
        });
    }

    /// Sends redacted Audiobookshelf book progress `json` only to clients
    /// that advertised `abs-book-progress` at Hello.
    pub(crate) fn broadcast_book_progress_gated(&mut self, json: String) {
        self.connection.retain(|c| {
            if !c.supports_abs_book_progress {
                return true;
            }
            c.tx.send(CtrlOutbound::Event(json.clone())).is_ok()
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
                    .map(|_| ack_rx)
            })
            .collect();
        let deadline = Instant::now() + timeout;
        for ack_rx in acks {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let _ = ack_rx.recv_timeout(remaining);
        }
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
