const SHARED_DATA_CONNECT_TIMEOUT: Duration = Duration::from_millis(750);
const SHARED_DATA_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const SHARED_DATA_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(15);

fn run_client_worker<S>(
    mut reader: BufReader<S>,
    cmd_rx: mpsc::Receiver<SharedDataCmd>,
    ev_tx: mpsc::Sender<SharedDataEvent>,
    heartbeat_supported: bool,
    heartbeat_interval: Duration,
    heartbeat_timeout: Duration,
) where
    S: Read + Write + Send + 'static,
{
    let mut line = String::new();
    let mut last_heartbeat = Instant::now();
    let mut awaiting_pong = false;

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            if !send_worker_command(&mut reader, &cmd) {
                log::warn!(target: "shared_data", "shared-data connection closed while sending command");
                let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                return;
            }
        }

        let now = Instant::now();
        if heartbeat_supported {
            if awaiting_pong && now.duration_since(last_heartbeat) >= heartbeat_timeout {
                log::warn!(target: "shared_data", "shared-data heartbeat timed out");
                let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                return;
            }
            if !awaiting_pong && now.duration_since(last_heartbeat) >= heartbeat_interval {
                if !send_worker_command(&mut reader, &SharedDataCmd::Ping) {
                    log::warn!(target: "shared_data", "shared-data connection closed while sending heartbeat");
                    let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                    return;
                }
                last_heartbeat = now;
                awaiting_pong = true;
            }
        }

        match reader.read_line(&mut line) {
            Ok(0) => {
                log::warn!(target: "shared_data", "shared-data server closed the connection");
                let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                return;
            }
            Ok(_) => {
                match serde_json::from_str::<SharedDataEvent>(line.trim()) {
                    Ok(event) => {
                        if matches!(event, SharedDataEvent::Pong) {
                            awaiting_pong = false;
                            last_heartbeat = Instant::now();
                        } else {
                            let _ = ev_tx.send(event);
                        }
                    }
                    Err(error) => log::warn!(
                        target: "shared_data",
                        "invalid shared-data response ({} bytes): {error}",
                        line.len()
                    ),
                }
                line.clear();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => {
                log::warn!(target: "shared_data", "shared-data read failed: {error}");
                let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                return;
            }
        }

        match cmd_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(cmd) => {
                if !send_worker_command(&mut reader, &cmd) {
                    log::warn!(target: "shared_data", "shared-data connection closed while sending command");
                    let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                return;
            }
        }
    }
}

fn send_worker_command<S: Write>(writer: &mut BufReader<S>, command: &SharedDataCmd) -> bool {
    let Ok(json) = serde_json::to_string(command) else {
        log::warn!(target: "shared_data", "failed to serialize shared-data command");
        return false;
    };
    if let SharedDataCmd::UpdateDocument { kind, .. } | SharedDataCmd::CreateDocument { kind, .. } =
        command
    {
        log::debug!(
            target: "shared_data",
            "sending shared-data document kind={} bytes={}",
            kind.as_str(),
            json.len()
        );
    }
    let mut message = json.into_bytes();
    message.push(b'\n');
    let mut offset = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    while offset < message.len() {
        match writer.get_mut().write(&message[offset..]) {
            Ok(0) => {
                log::warn!(target: "shared_data", "shared-data write returned zero bytes");
                return false;
            }
            Ok(written) => offset += written,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    log::warn!(target: "shared_data", "shared-data write timed out");
                    return false;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                log::warn!(target: "shared_data", "shared-data write failed: {error}");
                return false;
            }
        }
    }
    true
}

fn connect_tcp(addr: &str) -> Result<TcpStream, String> {
    let socket_addr = addr
        .to_socket_addrs()
        .map_err(|e| format!("resolve shared-data endpoint {addr}: {e}"))?
        .next()
        .ok_or_else(|| format!("shared-data endpoint has no address: {addr}"))?;
    TcpStream::connect_timeout(&socket_addr, SHARED_DATA_CONNECT_TIMEOUT)
        .map_err(|e| format!("connect to {addr}: {e}"))
}

fn with_request_id(command: SharedDataCmd, request_id: u64) -> SharedDataCmd {
    match command {
        SharedDataCmd::Snapshot { .. } => SharedDataCmd::Snapshot { request_id },
        SharedDataCmd::CreateDocument { kind, value, .. } => SharedDataCmd::CreateDocument {
            request_id,
            kind,
            value,
        },
        SharedDataCmd::UpdateDocument {
            kind,
            expected_revision,
            value,
            ..
        } => SharedDataCmd::UpdateDocument {
            request_id,
            kind,
            expected_revision,
            value,
        },
        SharedDataCmd::GetFeedEntry {
            feed_id,
            entry_guid,
            ..
        } => SharedDataCmd::GetFeedEntry {
            request_id,
            feed_id,
            entry_guid,
        },
        SharedDataCmd::PutFeedEntry {
            feed_id,
            entry_guid,
            value,
            ..
        } => SharedDataCmd::PutFeedEntry {
            request_id,
            feed_id,
            entry_guid,
            value,
        },
        SharedDataCmd::ScanFeedEntries { feed_id, .. } => SharedDataCmd::ScanFeedEntries {
            request_id,
            feed_id,
        },
        SharedDataCmd::Hello {
            auth_token,
            user_id,
        } => SharedDataCmd::Hello {
            auth_token,
            user_id,
        },
        SharedDataCmd::Ping => SharedDataCmd::Ping,
    }
}

fn event_request_id(event: &SharedDataEvent) -> Option<u64> {
    match event {
        SharedDataEvent::Snapshot { request_id, .. }
        | SharedDataEvent::DocumentCreated { request_id, .. }
        | SharedDataEvent::DocumentAlreadyExists { request_id, .. }
        | SharedDataEvent::DocumentUpdated { request_id, .. }
        | SharedDataEvent::DocumentStale { request_id, .. }
        | SharedDataEvent::FeedEntry { request_id, .. }
        | SharedDataEvent::FeedEntryAbsent { request_id, .. }
        | SharedDataEvent::FeedEntryPut { request_id, .. }
        | SharedDataEvent::FeedEntriesScanned { request_id, .. }
        | SharedDataEvent::RequestError { request_id, .. } => Some(*request_id),
        _ => None,
    }
}

fn read_line<S: BufRead>(reader: &mut S, label: &str) -> Result<String, String> {
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .map_err(|e| format!("read {label}: {e}"))?;
    if bytes == 0 {
        return Err(format!("connection closed before {label}"));
    }
    Ok(line.trim_end_matches(['\n', '\r']).to_string())
}

fn send_command<S: Write>(
    writer: &mut S,
    command: &SharedDataCmd,
    label: &str,
) -> Result<(), String> {
    let json = serde_json::to_string(command).map_err(|e| format!("serialize {label}: {e}"))?;
    writeln!(writer, "{json}").map_err(|e| format!("send {label}: {e}"))
}
