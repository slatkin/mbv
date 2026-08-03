enum MaybeTls {
    Plain(TcpStream),
    Unix(UnixStream),
    Tls(native_tls::TlsStream<TcpStream>),
}

const SHARED_DATA_CONNECT_TIMEOUT: Duration = Duration::from_millis(750);
const SHARED_DATA_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const SHARED_DATA_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(15);

impl std::io::Read for MaybeTls {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(buf),
            Self::Unix(s) => s.read(buf),
            Self::Tls(s) => s.read(buf),
        }
    }
}

impl std::io::Write for MaybeTls {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(buf),
            Self::Unix(s) => s.write(buf),
            Self::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Unix(s) => s.flush(),
            Self::Tls(s) => s.flush(),
        }
    }
}

impl MaybeTls {
    fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_read_timeout(dur),
            Self::Unix(s) => s.set_read_timeout(dur),
            Self::Tls(s) => s.get_ref().set_read_timeout(dur),
        }
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_write_timeout(dur),
            Self::Unix(s) => s.set_write_timeout(dur),
            Self::Tls(s) => s.get_ref().set_write_timeout(dur),
        }
    }

    fn set_nonblocking(&self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_nonblocking(true),
            Self::Unix(s) => s.set_nonblocking(true),
            Self::Tls(s) => s.get_ref().set_nonblocking(true),
        }
    }
}

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
                let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                return;
            }
        }

        let now = Instant::now();
        if heartbeat_supported {
            if awaiting_pong && now.duration_since(last_heartbeat) >= heartbeat_timeout {
                let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                return;
            }
            if !awaiting_pong && now.duration_since(last_heartbeat) >= heartbeat_interval {
                if !send_worker_command(&mut reader, &SharedDataCmd::Ping) {
                    let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                    return;
                }
                last_heartbeat = now;
                awaiting_pong = true;
            }
        }

        match reader.read_line(&mut line) {
            Ok(0) => {
                let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                return;
            }
            Ok(_) => {
                if let Ok(event) = serde_json::from_str::<SharedDataEvent>(line.trim()) {
                    if matches!(event, SharedDataEvent::Pong) {
                        awaiting_pong = false;
                        last_heartbeat = Instant::now();
                    } else {
                        let _ = ev_tx.send(event);
                    }
                }
                line.clear();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => {
                let _ = ev_tx.send(SharedDataEvent::ConnectionClosed);
                return;
            }
        }

        match cmd_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(cmd) => {
                if !send_worker_command(&mut reader, &cmd) {
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
        return false;
    };
    writeln!(writer.get_mut(), "{json}").is_ok()
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
        SharedDataCmd::Hello { auth_token } => SharedDataCmd::Hello { auth_token },
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
