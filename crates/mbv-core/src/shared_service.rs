use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{mpsc, Arc, Mutex};

use crate::api::EmbyClient;
use crate::shared_protocol::{SharedDataCmd, SharedDataEvent, SharedDataHello};
use crate::shared_state::SharedDocumentKind;
use crate::shared_worker::SharedStoreHandle;

/// Transport type for a shared-data connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedTransport {
    Local,
    Tcp,
    Tls,
}

/// Registry of active shared-data sessions, used for post-commit fan-out.
struct SharedSession {
    id: u64,
    user_id: String,
    tx: mpsc::Sender<String>,
    #[allow(dead_code)]
    transport: SharedTransport,
}

pub struct SharedSessions {
    next_id: u64,
    sessions: Vec<SharedSession>,
}

/// Registry of active shared-data sessions, used for post-commit fan-out.
pub type SharedSessionRegistry = Arc<Mutex<SharedSessions>>;

/// Bind the shared-data Unix domain listener.
fn bind_shared_unix_listener(path: &str) -> Option<UnixListener> {
    let _ = std::fs::remove_file(path);
    match UnixListener::bind(path) {
        Ok(listener) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            }
            Some(listener)
        }
        Err(e) => {
            log::error!(
                target: "shared_data",
                "shared-data unix socket bind failed: {e}"
            );
            None
        }
    }
}

/// Build the optional TLS acceptor for a shared-data TCP listener.
fn bind_shared_tls_acceptor(cert_path: &str, key_path: &str) -> Option<native_tls::TlsAcceptor> {
    let cert = match std::fs::read(cert_path) {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                target: "shared_data",
                "failed to read TLS cert {cert_path}: {e}"
            );
            return None;
        }
    };
    let key = match std::fs::read(key_path) {
        Ok(k) => k,
        Err(e) => {
            log::error!(
                target: "shared_data",
                "failed to read TLS key {key_path}: {e}"
            );
            return None;
        }
    };

    let identity = match native_tls::Identity::from_pkcs8(&cert, &key) {
        Ok(id) => id,
        Err(e) => {
            log::error!(
                target: "shared_data",
                "failed to load TLS identity from {cert_path}/{key_path}: {e}"
            );
            return None;
        }
    };

    let acceptor = match native_tls::TlsAcceptor::new(identity) {
        Ok(a) => a,
        Err(e) => {
            log::error!(
                target: "shared_data",
                "failed to create TLS acceptor: {e}"
            );
            return None;
        }
    };

    Some(acceptor)
}

trait SharedStream: std::io::Read + Write + Send + Sized + 'static {
    fn set_read_timeout_stream(&self, timeout: Option<std::time::Duration>) -> std::io::Result<()>;
    fn set_write_timeout_stream(&self, timeout: Option<std::time::Duration>)
        -> std::io::Result<()>;
    fn set_nonblocking_stream(&self) -> std::io::Result<()>;
}

impl SharedStream for UnixStream {
    fn set_read_timeout_stream(&self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        self.set_read_timeout(timeout)
    }

    fn set_write_timeout_stream(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.set_write_timeout(timeout)
    }

    fn set_nonblocking_stream(&self) -> std::io::Result<()> {
        self.set_nonblocking(true)
    }
}

impl SharedStream for std::net::TcpStream {
    fn set_read_timeout_stream(&self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        self.set_read_timeout(timeout)
    }

    fn set_write_timeout_stream(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.set_write_timeout(timeout)
    }

    fn set_nonblocking_stream(&self) -> std::io::Result<()> {
        self.set_nonblocking(true)
    }
}

impl SharedStream for native_tls::TlsStream<std::net::TcpStream> {
    fn set_read_timeout_stream(&self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        self.get_ref().set_read_timeout(timeout)
    }

    fn set_write_timeout_stream(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.get_ref().set_write_timeout(timeout)
    }

    fn set_nonblocking_stream(&self) -> std::io::Result<()> {
        self.get_ref().set_nonblocking(true)
    }
}

/// Start the shared-data service. Returns `None` if hosting is disabled or
/// the listener cannot be bound.
pub fn start_shared_service(
    client: Arc<Mutex<EmbyClient>>,
    store: SharedStoreHandle,
    config: &crate::config::Config,
) -> Option<SharedSessionRegistry> {
    if !config.shared_data_enabled {
        return None;
    }
    if let Err(error) = crate::config::validate_shared_data_config(config) {
        log::error!(target: "shared_data", "shared-data configuration rejected: {error}");
        return None;
    }

    let registry: SharedSessionRegistry = Arc::new(Mutex::new(SharedSessions {
        next_id: 0,
        sessions: Vec::new(),
    }));

    let listen = crate::config::shared_tcp_address(config.shared_data_listen.trim());
    let is_unix = listen.starts_with('/') || listen.starts_with("unix://");
    let is_tcp = !is_unix;

    if is_tcp {
        let listener = TcpListener::bind(listen).ok()?;
        let use_tls = !config.shared_data_tls_cert_path.trim().is_empty();
        let acceptor = if use_tls {
            Some(bind_shared_tls_acceptor(
                &config.shared_data_tls_cert_path,
                &config.shared_data_tls_key_path,
            )?)
        } else {
            None
        };

        log::info!(
            target: "shared_data",
            "shared-data {} listener on {}",
            if use_tls { "TLS" } else { "plaintext LAN" },
            listener.local_addr().map(|a| a.to_string()).unwrap_or_else(|_| listen.to_string())
        );

        let registry = registry.clone();
        let client = client.clone();
        let store = store.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                if let Some(acceptor) = &acceptor {
                    match acceptor.accept(stream) {
                        Ok(tls_stream) => spawn_shared_client_handler(
                            tls_stream,
                            SharedTransport::Tls,
                            registry.clone(),
                            client.clone(),
                            store.clone(),
                        ),
                        Err(e) => log::warn!(
                            target: "shared_data",
                            "shared-data TLS handshake failed: {e}"
                        ),
                    }
                } else {
                    spawn_shared_client_handler(
                        stream,
                        SharedTransport::Tcp,
                        registry.clone(),
                        client.clone(),
                        store.clone(),
                    );
                }
            }
        });
    } else {
        // Unix domain socket listener
        let listener = bind_shared_unix_listener(listen)?;

        log::info!(
            target: "shared_data",
            "shared-data Unix listener on {}",
            listen
        );

        let registry = registry.clone();
        let client = client.clone();
        let store = store.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                spawn_shared_client_handler(
                    stream,
                    SharedTransport::Local,
                    registry.clone(),
                    client.clone(),
                    store.clone(),
                );
            }
        });
    }

    Some(registry)
}

fn spawn_shared_client_handler<S>(
    stream: S,
    transport: SharedTransport,
    registry: SharedSessionRegistry,
    client: Arc<Mutex<EmbyClient>>,
    store: SharedStoreHandle,
) where
    S: SharedStream,
{
    std::thread::spawn(move || {
        let reader = Arc::new(Mutex::new(BufReader::new(stream)));
        let stream_ref = reader.lock().unwrap();
        if stream_ref
            .get_ref()
            .set_read_timeout_stream(Some(std::time::Duration::from_secs(10)))
            .is_err()
            || stream_ref
                .get_ref()
                .set_write_timeout_stream(Some(std::time::Duration::from_secs(5)))
                .is_err()
        {
            return;
        }
        drop(stream_ref);

        let (ev_tx, ev_rx) = mpsc::channel::<String>();
        let writer_reader = reader.clone();

        // Send daemon hello.
        let hello = SharedDataEvent::Hello(SharedDataHello::current());
        if let Ok(json) = serde_json::to_string(&hello) {
            if writeln!(reader.lock().unwrap().get_mut(), "{json}").is_err() {
                return;
            }
        }

        // Writer thread.
        std::thread::spawn(move || {
            for line in ev_rx {
                let mut reader = writer_reader.lock().unwrap();
                if writeln!(reader.get_mut(), "{line}").is_err() {
                    break;
                }
            }
            let _ = writer_reader.lock().unwrap().get_mut().flush();
        });

        // Reader: protocol handshake.
        // First line must be Hello with auth token.
        let mut line_buffer = String::new();
        let line = loop {
            match read_shared_line(&reader, &mut line_buffer) {
                Some(Ok(line)) => break line,
                Some(Err(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Some(Err(_)) | None => return,
            }
        };
        let cmd: SharedDataCmd = match serde_json::from_str(&line) {
            Ok(c) => c,
            Err(e) => {
                let _ = ev_tx.send(
                    serde_json::to_string(&SharedDataEvent::Error {
                        reason: format!("invalid handshake: {e}"),
                    })
                    .unwrap_or_default(),
                );
                return;
            }
        };

        let auth_token = match cmd {
            SharedDataCmd::Hello { auth_token } => auth_token,
            _ => {
                let _ = ev_tx.send(
                    serde_json::to_string(&SharedDataEvent::Error {
                        reason: "expected Hello as first message".to_string(),
                    })
                    .unwrap_or_default(),
                );
                return;
            }
        };

        // Validate the Emby token — /Users/Me only, no API-key fallback.
        let validate_client = client.lock().unwrap().clone();
        let user_id = match validate_client.validate_shared_data_token(&auth_token) {
            Ok(uid) => uid,
            Err(e) => {
                let _ = ev_tx.send(
                    serde_json::to_string(&SharedDataEvent::AuthFailed { reason: e })
                        .unwrap_or_default(),
                );
                return;
            }
        };
        // Token discarded here — never stored, logged, or persisted.

        let _ = ev_tx.send(
            serde_json::to_string(&SharedDataEvent::AuthOk {
                user_id: user_id.clone(),
            })
            .unwrap_or_default(),
        );

        if reader
            .lock()
            .unwrap()
            .get_ref()
            .set_nonblocking_stream()
            .is_err()
        {
            return;
        }

        // Register session.
        let session_id = {
            let mut reg = registry.lock().unwrap();
            let id = reg.next_id;
            reg.next_id += 1;
            reg.sessions.push(SharedSession {
                id,
                user_id: user_id.clone(),
                tx: ev_tx.clone(),
                transport,
            });
            id
        };

        // Command loop.
        loop {
            let line = match read_shared_line(&reader, &mut line_buffer) {
                Some(Ok(line)) => line,
                Some(Err(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                Some(Err(_)) | None => break,
            };
            if line.is_empty() {
                continue;
            }
            let cmd: SharedDataCmd = match serde_json::from_str(&line) {
                Ok(c) => c,
                Err(e) => {
                    let _ = ev_tx.send(
                        serde_json::to_string(&SharedDataEvent::Error {
                            reason: format!("invalid command: {e}"),
                        })
                        .unwrap_or_default(),
                    );
                    continue;
                }
            };

            match cmd {
                SharedDataCmd::Hello { .. } => {
                    let _ = ev_tx.send(
                        serde_json::to_string(&SharedDataEvent::Error {
                            reason: "unexpected re-hello".to_string(),
                        })
                        .unwrap_or_default(),
                    );
                }
                SharedDataCmd::Ping => {
                    let _ = ev_tx
                        .send(serde_json::to_string(&SharedDataEvent::Pong).unwrap_or_default());
                }
                SharedDataCmd::Snapshot { request_id } => match store.read_all(&user_id) {
                    Ok((q, l, r, s)) => {
                        let ev = SharedDataEvent::Snapshot {
                            request_id,
                            queue_state: q,
                            library_position_state: l,
                            last_remote_connection: r,
                            roaming_settings: s,
                        };
                        let _ = ev_tx.send(serde_json::to_string(&ev).unwrap_or_default());
                    }
                    Err(e) => {
                        let _ = ev_tx.send(
                            serde_json::to_string(&SharedDataEvent::RequestError {
                                request_id,
                                reason: e,
                            })
                            .unwrap_or_default(),
                        );
                    }
                },
                SharedDataCmd::CreateDocument {
                    request_id,
                    kind,
                    value,
                } => {
                    match store.create(&user_id, kind, value) {
                        Ok(record) => {
                            let ev = SharedDataEvent::DocumentCreated {
                                request_id,
                                kind,
                                record: record.clone(),
                            };
                            let _ = ev_tx.send(serde_json::to_string(&ev).unwrap_or_default());
                            // Fan out to other same-user sessions.
                            fan_out_notification(&registry, session_id, &user_id, kind, &record);
                        }
                        Err(e) if e.starts_with("already_exists:") => {
                            let current: crate::shared_state::SharedRecord =
                                serde_json::from_str(&e["already_exists:".len()..])
                                    .unwrap_or_default();
                            let _ = ev_tx.send(
                                serde_json::to_string(&SharedDataEvent::DocumentAlreadyExists {
                                    request_id,
                                    kind,
                                    current,
                                })
                                .unwrap_or_default(),
                            );
                        }
                        Err(e) => {
                            let _ = ev_tx.send(
                                serde_json::to_string(&SharedDataEvent::RequestError {
                                    request_id,
                                    reason: e,
                                })
                                .unwrap_or_default(),
                            );
                        }
                    }
                }
                SharedDataCmd::UpdateDocument {
                    request_id,
                    kind,
                    expected_revision,
                    value,
                } => {
                    match store.update(&user_id, kind, expected_revision, value) {
                        Ok(record) => {
                            let ev = SharedDataEvent::DocumentUpdated {
                                request_id,
                                kind,
                                record: record.clone(),
                            };
                            let _ = ev_tx.send(serde_json::to_string(&ev).unwrap_or_default());
                            // Fan out to other same-user sessions.
                            fan_out_notification(&registry, session_id, &user_id, kind, &record);
                        }
                        Err(e) if e.starts_with("stale:") => {
                            let current: crate::shared_state::SharedRecord =
                                serde_json::from_str(&e["stale:".len()..]).unwrap_or_default();
                            let _ = ev_tx.send(
                                serde_json::to_string(&SharedDataEvent::DocumentStale {
                                    request_id,
                                    kind,
                                    current,
                                })
                                .unwrap_or_default(),
                            );
                        }
                        Err(e) => {
                            let _ = ev_tx.send(
                                serde_json::to_string(&SharedDataEvent::RequestError {
                                    request_id,
                                    reason: e,
                                })
                                .unwrap_or_default(),
                            );
                        }
                    }
                }
            }
        }

        // Unregister session on disconnect.
        registry
            .lock()
            .unwrap()
            .sessions
            .retain(|s| s.id != session_id);
    });
}

fn read_shared_line<S: SharedStream>(
    reader: &Arc<Mutex<BufReader<S>>>,
    line: &mut String,
) -> Option<std::io::Result<String>> {
    let result = reader.lock().unwrap().read_line(line);
    match result {
        Ok(0) => None,
        Ok(_) => Some(Ok(std::mem::take(line)
            .trim_end_matches(['\n', '\r'])
            .to_string())),
        Err(error) => Some(Err(error)),
    }
}

/// Send a document notification to all other same-user sessions.
fn fan_out_notification(
    registry: &SharedSessionRegistry,
    exclude_id: u64,
    user_id: &str,
    kind: SharedDocumentKind,
    record: &crate::shared_state::SharedRecord,
) {
    let ev = SharedDataEvent::DocumentNotification {
        kind,
        record: record.clone(),
    };
    let json = match serde_json::to_string(&ev) {
        Ok(j) => j,
        Err(_) => return,
    };
    let reg = registry.lock().unwrap();
    for session in &reg.sessions {
        if session.id != exclude_id && session.user_id == user_id {
            let _ = session.tx.send(json.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::read_shared_line;
    use std::io::Write;
    use std::os::unix::net::UnixListener;
    use std::sync::{Arc, Mutex};

    #[test]
    fn nonblocking_partial_line_is_preserved() {
        let path =
            std::env::temp_dir().join(format!("mbv-shared-framing-{}.sock", uuid::Uuid::new_v4()));
        let listener = UnixListener::bind(&path).unwrap();
        let client = std::os::unix::net::UnixStream::connect(&path).unwrap();
        let (server, _) = listener.accept().unwrap();
        server.set_nonblocking(true).unwrap();
        let reader = Arc::new(Mutex::new(std::io::BufReader::new(server)));
        let mut pending = String::new();

        let mut client = client;
        client.write_all(b"{\"partial\":").unwrap();
        assert!(matches!(
            read_shared_line(&reader, &mut pending),
            Some(Err(error)) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
        assert_eq!(pending, "{\"partial\":");

        client.write_all(b"true}\n").unwrap();
        assert_eq!(
            read_shared_line(&reader, &mut pending).unwrap().unwrap(),
            "{\"partial\":true}"
        );
        assert!(pending.is_empty());
        let _ = std::fs::remove_file(path);
    }
}
