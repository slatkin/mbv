use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::api::EmbyClient;
use crate::config::Config;
use crate::shared_protocol::{SharedDataCmd, SharedDataEvent};
use crate::shared_state::{SharedDocumentKind, SharedRecord, SharedSnapshotResponse};

/// State machine for the shared-data client connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SharedClientState {
    Disabled,
    Configured,
    Connecting,
    Shared,
    LocalFallback,
}

/// Per-document revision tracker while connected.
#[derive(Default, Clone, Debug)]
pub struct DocumentRevisions {
    pub queue_state: Option<u64>,
    pub library_position_state: Option<u64>,
    pub last_remote_connection: Option<u64>,
    pub roaming_settings: Option<u64>,
}

include!("shared_client_transport.rs");

/// Handle to the shared-data client connection.
pub struct SharedClient {
    state: SharedClientState,
    user_id: Option<String>,
    revisions: DocumentRevisions,
    tx: Option<mpsc::Sender<SharedDataCmd>>,
    event_rx: Option<mpsc::Receiver<SharedDataEvent>>,
    pending_events: VecDeque<SharedDataEvent>,
    next_request_id: u64,
    backoff: Duration,
    last_attempt: Option<Instant>,
}

impl Default for SharedClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedClient {
    pub fn new() -> Self {
        Self {
            state: SharedClientState::Disabled,
            user_id: None,
            revisions: DocumentRevisions::default(),
            tx: None,
            event_rx: None,
            pending_events: VecDeque::new(),
            next_request_id: 1,
            backoff: Duration::from_secs(1),
            last_attempt: None,
        }
    }

    pub fn state(&self) -> &SharedClientState {
        &self.state
    }

    pub fn user_id(&self) -> Option<&str> {
        self.user_id.as_deref()
    }

    pub fn revisions(&self) -> &DocumentRevisions {
        &self.revisions
    }

    /// Initialize from configuration.
    pub fn initialize(&mut self, config: &Config) {
        let endpoint = config.shared_data_endpoint.trim();
        if !config.shared_data_enabled && endpoint.is_empty() {
            self.state = SharedClientState::Disabled;
            return;
        }
        if !endpoint.is_empty() {
            if let Err(e) = crate::config::validate_shared_data_endpoint(endpoint) {
                log::warn!(target: "shared_data", "shared-data endpoint rejected: {e}");
                self.state = SharedClientState::Disabled;
                return;
            }
        }
        self.state = SharedClientState::Configured;
    }

    /// Attempt to connect and authenticate. Returns the initial snapshot.
    pub fn connect(
        &mut self,
        config: &Config,
        client: &EmbyClient,
    ) -> Result<SharedSnapshotResponse, String> {
        let endpoint = if config.shared_data_endpoint.trim().is_empty() {
            discover_shared_data_endpoint(client)?
        } else {
            config.shared_data_endpoint.trim().to_string()
        };
        let endpoint = endpoint.as_str();

        self.state = SharedClientState::Connecting;
        self.last_attempt = Some(Instant::now());

        let stream = if let Some(addr) = endpoint.strip_prefix("tcp://") {
            MaybeTls::Plain(connect_tcp(addr)?)
        } else if endpoint.starts_with("unix://") || endpoint.starts_with('/') {
            let path = endpoint.strip_prefix("unix://").unwrap_or(endpoint);
            MaybeTls::Unix(
                UnixStream::connect(path)
                    .map_err(|e| format!("connect to shared-data socket {path}: {e}"))?,
            )
        } else if let Some(addr) = endpoint.strip_prefix("tls://") {
            let connector =
                native_tls::TlsConnector::new().map_err(|e| format!("TLS connector: {e}"))?;
            let tcp = connect_tcp(addr)?;
            let server_name = addr
                .rsplit_once(':')
                .map(|(host, _)| host.trim_matches(['[', ']']))
                .filter(|host| !host.is_empty())
                .unwrap_or(addr);
            MaybeTls::Tls(
                connector
                    .connect(server_name, tcp)
                    .map_err(|e| format!("TLS handshake with {addr}: {e}"))?,
            )
        } else {
            return Err(format!(
                "unsupported shared-data endpoint scheme: {endpoint}"
            ));
        };

        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| format!("set read timeout: {e}"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("set write timeout: {e}"))?;

        // Keep one full-duplex stream. TLS streams cannot be cloned into
        // independent reader and writer streams without creating a second
        // TLS session, so the post-handshake worker owns both directions.
        let mut reader = BufReader::new(stream);

        let hello_line = read_line(&mut reader, "daemon hello")?;
        let hello: SharedDataEvent =
            serde_json::from_str(&hello_line).map_err(|e| format!("parse daemon hello: {e}"))?;
        let (heartbeat_supported, user_id_auth_supported) = match hello {
            SharedDataEvent::Hello(h) => {
                if !h
                    .capabilities
                    .iter()
                    .any(|cap| cap == crate::shared_protocol::SHARED_DATA_CAP_V1)
                {
                    return Err("daemon does not support shared-mbv-state-v1".to_string());
                }
                (
                    h.capabilities
                        .iter()
                        .any(|cap| cap == crate::shared_protocol::SHARED_DATA_CAP_HEARTBEAT_V1),
                    h.capabilities
                        .iter()
                        .any(|cap| cap == crate::shared_protocol::SHARED_DATA_CAP_USER_ID_AUTH_V1),
                )
            }
            other => {
                let s = serde_json::to_string(&other).unwrap_or_default();
                return Err(format!("expected Hello, got: {s}"));
            }
        };

        let hello_cmd = SharedDataCmd::Hello {
            auth_token: client.token.clone(),
            user_id: user_id_auth_supported.then(|| client.user_id.clone()),
        };
        send_command(reader.get_mut(), &hello_cmd, "hello")?;

        let auth_line = read_line(&mut reader, "auth response")?;
        let auth_resp: SharedDataEvent =
            serde_json::from_str(&auth_line).map_err(|e| format!("parse auth response: {e}"))?;
        match auth_resp {
            SharedDataEvent::AuthOk { user_id } => self.user_id = Some(user_id),
            SharedDataEvent::AuthFailed { reason } => {
                return Err(format!("authentication failed: {reason}"));
            }
            other => {
                let s = serde_json::to_string(&other).unwrap_or_default();
                return Err(format!("expected AuthOk/AuthFailed, got: {s}"));
            }
        }

        let snapshot_request_id = self.allocate_request_id();
        send_command(
            reader.get_mut(),
            &SharedDataCmd::Snapshot {
                request_id: snapshot_request_id,
            },
            "snapshot",
        )?;
        let snapshot_resp = loop {
            let snapshot_line = read_line(&mut reader, "snapshot")?;
            let event: SharedDataEvent =
                serde_json::from_str(&snapshot_line).map_err(|e| format!("parse snapshot: {e}"))?;
            if event_request_id(&event) == Some(snapshot_request_id) {
                break event;
            }
            self.pending_events.push_back(event);
        };

        let snapshot = match snapshot_resp {
            SharedDataEvent::Snapshot {
                request_id: _,
                queue_state,
                library_position_state,
                last_remote_connection,
                roaming_settings,
            } => {
                self.revisions.queue_state = queue_state.as_ref().map(|r| r.revision);
                self.revisions.library_position_state =
                    library_position_state.as_ref().map(|r| r.revision);
                self.revisions.last_remote_connection =
                    last_remote_connection.as_ref().map(|r| r.revision);
                self.revisions.roaming_settings = roaming_settings.as_ref().map(|r| r.revision);
                SharedSnapshotResponse {
                    queue_state,
                    library_position_state,
                    last_remote_connection,
                    roaming_settings,
                }
            }
            SharedDataEvent::RequestError { reason, .. } => {
                return Err(format!("snapshot error: {reason}"));
            }
            other => {
                let s = serde_json::to_string(&other).unwrap_or_default();
                return Err(format!("expected Snapshot, got: {s}"));
            }
        };

        reader
            .get_ref()
            .set_nonblocking()
            .map_err(|e| format!("set shared-data stream nonblocking: {e}"))?;
        let (cmd_tx, cmd_rx) = mpsc::channel::<SharedDataCmd>();
        let (ev_tx, ev_rx) = mpsc::channel::<SharedDataEvent>();
        std::thread::spawn(move || {
            run_client_worker(
                reader,
                cmd_rx,
                ev_tx,
                heartbeat_supported,
                SHARED_DATA_HEARTBEAT_INTERVAL,
                SHARED_DATA_HEARTBEAT_TIMEOUT,
            );
        });

        self.tx = Some(cmd_tx);
        self.event_rx = Some(ev_rx);
        self.state = SharedClientState::Shared;
        self.backoff = Duration::from_secs(1);
        Ok(snapshot)
    }

    /// Initialize absent shared documents from local records. Existing shared
    /// documents always win; a create race is resolved by adopting the
    /// server's `DocumentAlreadyExists` response.
    pub fn initialize_missing_documents(
        &mut self,
        mut snapshot: SharedSnapshotResponse,
        local: &SharedSnapshotResponse,
    ) -> Result<SharedSnapshotResponse, String> {
        let record = self.initialize_one(
            SharedDocumentKind::QueueState,
            snapshot.queue_state.as_ref(),
            local.queue_state.as_ref(),
        )?;
        if snapshot.queue_state.is_none() {
            snapshot.queue_state = record;
        }

        let record = self.initialize_one(
            SharedDocumentKind::LibraryPositionState,
            snapshot.library_position_state.as_ref(),
            local.library_position_state.as_ref(),
        )?;
        if snapshot.library_position_state.is_none() {
            snapshot.library_position_state = record;
        }

        let record = self.initialize_one(
            SharedDocumentKind::LastRemoteConnection,
            snapshot.last_remote_connection.as_ref(),
            local.last_remote_connection.as_ref(),
        )?;
        if snapshot.last_remote_connection.is_none() {
            snapshot.last_remote_connection = record;
        }

        let record = self.initialize_one(
            SharedDocumentKind::RoamingSettings,
            snapshot.roaming_settings.as_ref(),
            local.roaming_settings.as_ref(),
        )?;
        if snapshot.roaming_settings.is_none() {
            snapshot.roaming_settings = record;
        }

        Ok(snapshot)
    }

    fn initialize_one(
        &mut self,
        kind: SharedDocumentKind,
        shared: Option<&SharedRecord>,
        local: Option<&SharedRecord>,
    ) -> Result<Option<SharedRecord>, String> {
        if shared.is_some() {
            return Ok(None);
        }
        let Some(local) = local else {
            return Ok(None);
        };
        let event = self.request_and_wait(SharedDataCmd::CreateDocument {
            request_id: 0,
            kind,
            value: local.value.clone(),
        })?;
        let record = match event {
            SharedDataEvent::DocumentCreated {
                request_id: _,
                kind: response_kind,
                record,
            } if response_kind == kind => record,
            SharedDataEvent::DocumentAlreadyExists {
                request_id: _,
                kind: response_kind,
                current,
            } if response_kind == kind => current,
            SharedDataEvent::RequestError { reason, .. } => {
                return Err(format!("initialize {}: {reason}", kind.as_str()));
            }
            other => {
                return Err(format!(
                    "initialize {} received unexpected response: {}",
                    kind.as_str(),
                    serde_json::to_string(&other).unwrap_or_default()
                ));
            }
        };
        self.set_revision(kind, record.revision);
        Ok(Some(record))
    }

    fn request_and_wait(&mut self, command: SharedDataCmd) -> Result<SharedDataEvent, String> {
        let request_id = self.allocate_request_id();
        let command = with_request_id(command, request_id);
        let tx = self.tx.as_ref().ok_or("not connected")?;
        tx.send(command)
            .map_err(|_| "shared-data connection is closed".to_string())?;
        loop {
            if let Some(index) = self
                .pending_events
                .iter()
                .position(|event| event_request_id(event) == Some(request_id))
            {
                return Ok(self.pending_events.remove(index).unwrap());
            }
            let event = {
                let Some(rx) = &self.event_rx else {
                    return Err("shared-data event channel is unavailable".to_string());
                };
                match rx.recv_timeout(Duration::from_secs(10)) {
                    Ok(event) => event,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        return Err("timed out waiting for shared-data write response".to_string());
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err("shared-data connection closed".to_string());
                    }
                }
            };
            if matches!(event, SharedDataEvent::ConnectionClosed) {
                return Err("shared-data connection closed".to_string());
            }
            if event_request_id(&event) == Some(request_id) {
                return Ok(event);
            }
            self.pending_events.push_back(event);
        }
    }

    fn allocate_request_id(&mut self) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        request_id
    }

    fn set_revision(&mut self, kind: SharedDocumentKind, revision: u64) {
        match kind {
            SharedDocumentKind::QueueState => self.revisions.queue_state = Some(revision),
            SharedDocumentKind::LibraryPositionState => {
                self.revisions.library_position_state = Some(revision)
            }
            SharedDocumentKind::LastRemoteConnection => {
                self.revisions.last_remote_connection = Some(revision)
            }
            SharedDocumentKind::RoamingSettings => self.revisions.roaming_settings = Some(revision),
        }
    }

    /// Send an update request.
    pub fn update_document(
        &mut self,
        kind: SharedDocumentKind,
        expected_revision: u64,
        value: serde_json::Value,
    ) -> Result<(), String> {
        let request_id = self.allocate_request_id();
        let command = with_request_id(
            SharedDataCmd::UpdateDocument {
                request_id: 0,
                kind,
                expected_revision,
                value,
            },
            request_id,
        );
        let tx = self.tx.as_ref().ok_or("not connected")?;
        tx.send(command).map_err(|_| "send failed".to_string())
    }

    /// Send a create request.
    pub fn create_document(
        &mut self,
        kind: SharedDocumentKind,
        value: serde_json::Value,
    ) -> Result<(), String> {
        let request_id = self.allocate_request_id();
        let command = with_request_id(
            SharedDataCmd::CreateDocument {
                request_id: 0,
                kind,
                value,
            },
            request_id,
        );
        let tx = self.tx.as_ref().ok_or("not connected")?;
        tx.send(command).map_err(|_| "send failed".to_string())
    }

    /// Update a document and wait for its durable acknowledgement. A stale
    /// response returns the authoritative current record without retrying.
    pub fn update_document_and_wait(
        &mut self,
        kind: SharedDocumentKind,
        expected_revision: u64,
        value: serde_json::Value,
    ) -> Result<SharedWriteResult, String> {
        let event = self.request_and_wait(SharedDataCmd::UpdateDocument {
            request_id: 0,
            kind,
            expected_revision,
            value,
        })?;
        match event {
            SharedDataEvent::DocumentUpdated {
                request_id: _,
                kind: response_kind,
                record,
            } if response_kind == kind => {
                self.set_revision(kind, record.revision);
                Ok(SharedWriteResult::Committed(record))
            }
            SharedDataEvent::DocumentStale {
                request_id: _,
                kind: response_kind,
                current,
            } if response_kind == kind => {
                self.set_revision(kind, current.revision);
                Ok(SharedWriteResult::Stale(current))
            }
            SharedDataEvent::RequestError { reason, .. } => Err(reason),
            other => Err(format!(
                "update {} received unexpected response: {}",
                kind.as_str(),
                serde_json::to_string(&other).unwrap_or_default()
            )),
        }
    }

    /// Drain pending events from the background reader.
    pub fn drain_events(&mut self) -> Vec<SharedDataEvent> {
        let mut events: Vec<SharedDataEvent> = std::mem::take(&mut self.pending_events)
            .into_iter()
            .collect();
        if let Some(rx) = &self.event_rx {
            events.extend(rx.try_iter());
        }
        events
    }

    /// Enter local fallback state.
    pub fn enter_fallback(&mut self) {
        self.state = SharedClientState::LocalFallback;
        self.last_attempt = Some(Instant::now());
        self.backoff = (self.backoff * 2).min(Duration::from_secs(60));
    }

    /// Record a failed background reconnect attempt.
    pub fn record_retry_failure(&mut self) {
        self.state = SharedClientState::LocalFallback;
        self.last_attempt = Some(Instant::now());
        self.backoff = (self.backoff * 2).min(Duration::from_secs(60));
    }

    /// Check if a reconnection attempt is due.
    pub fn should_retry(&self) -> bool {
        matches!(
            self.state,
            SharedClientState::LocalFallback | SharedClientState::Configured
        ) && self
            .last_attempt
            .map(|a| a.elapsed() >= self.backoff + Duration::from_millis(self.retry_jitter_ms()))
            .unwrap_or(true)
    }

    fn retry_jitter_ms(&self) -> u64 {
        let ceiling = (self.backoff.as_millis() as u64 / 4).min(250);
        if ceiling == 0 {
            return 0;
        }
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_millis() as u64 % (ceiling + 1))
            .unwrap_or(0)
    }

    /// Reset backoff after a successful connection.
    pub fn reset_backoff(&mut self) {
        self.backoff = Duration::from_secs(1);
    }

    /// Apply a committed notification: update the revision tracker.
    pub fn apply_notification(&mut self, kind: SharedDocumentKind, record: &SharedRecord) {
        self.set_revision(kind, record.revision);
    }
}

fn discover_shared_data_endpoint(client: &EmbyClient) -> Result<String, String> {
    let endpoints: Vec<String> = client
        .get_sessions()?
        .into_iter()
        .filter_map(|session| {
            crate::api::parse_mbv_shared_data_tcp_port(&session.supported_commands)
                .map(|port| format!("tcp://{}:{port}", session.host))
        })
        .filter(|endpoint| crate::config::validate_shared_data_endpoint(endpoint).is_ok())
        .collect();
    match endpoints.as_slice() {
        [endpoint] => Ok(endpoint.clone()),
        [] => Err("no shared-data-enabled mbvd found for this Emby account".to_string()),
        _ => Err("multiple shared-data-enabled mbvd hosts found; configure shared_data.endpoint to select one".to_string()),
    }
}

pub enum SharedWriteResult {
    Committed(SharedRecord),
    Stale(SharedRecord),
}

#[cfg(test)]
mod tests {
    include!("shared_client_tests.rs");
}
