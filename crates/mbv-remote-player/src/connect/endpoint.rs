//! Daemon endpoint parsing and connection. Split out of `remote_player/connect.rs`
//! so that module stays under the size bar.

use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use crate::RemotePlayerError;
use mbv_net::stream::SocketStream;

const DAEMON_TCP_CONNECT_TIMEOUT: Duration = Duration::from_millis(750);

// A local daemon that was *just* launched (via `stay_alive` or auto-detect)
// may have written its PID file (which is what makes it "detected") slightly
// before its ctrl socket is bound. Retry briefly rather than immediately
// falling back to standalone. Explicit remote endpoints (`Unix(path)` /
// `Tcp`) are not retried this way — they represent an already-running,
// user-specified target, not a same-machine process that might still be
// starting up.
const LOCAL_DAEMON_CONNECT_RETRY_TIMEOUT: Duration = Duration::from_secs(1);
const LOCAL_DAEMON_CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// Resolves the configured endpoint for a library name (#256). Matches
/// case-insensitively (the query is lowercased before lookup; `routes`'
/// keys are already lowercased by `parse_config`). No wildcard fallback --
/// returns `None` if the library has no route, and the caller stays local.
///
/// Parses the stored string via `DaemonEndpoint::parse` and requires it to
/// be `Tcp(_)` -- library routing is a remote-only feature (#239 addendum:
/// "#222 and #223 are remote-connection features only"), so anything else
/// is malformed: a bare pre-#256 device-name string (which `parse` would
/// otherwise silently accept as a bogus `Unix(PathBuf)` socket path), a
/// `unix://` value, or a bare `local`/empty value are all logged and
/// skipped rather than routed. This is a pure, synchronous, no-network
/// lookup -- the entire point of #256 is that route resolution on the
/// play/enqueue path never touches `/Sessions` again.
#[must_use]
pub fn resolve_library_route(
    routes: &std::collections::BTreeMap<String, String>,
    library_name: &str,
) -> Option<DaemonEndpoint> {
    let raw = routes.get(&library_name.to_lowercase())?;
    match DaemonEndpoint::parse(raw) {
        Ok(endpoint @ DaemonEndpoint::Tcp(_)) => Some(endpoint),
        Ok(other) => {
            log::warn!(
                target: "library_route",
                "library_routes entry {raw:?} parsed as {other:?}, but library routing is tcp://-only; skipping"
            );
            None
        }
        Err(e) => {
            log::warn!(
                target: "library_route",
                "library_routes entry {raw:?} is not a valid tcp:// endpoint: {e}; skipping"
            );
            None
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DaemonEndpoint {
    Local,
    Unix(PathBuf),
    Tcp(SocketAddr),
}

impl DaemonEndpoint {
    pub fn parse(value: &str) -> Result<Self, crate::RemotePlayerError> {
        let value = value.trim();
        if value.is_empty() || value == "local" {
            return Ok(Self::Local);
        }
        if let Some(path) = value.strip_prefix("unix://") {
            if path.is_empty() {
                return Err(RemotePlayerError::endpoint(
                    "daemon endpoint unix:// requires a socket path",
                ));
            }
            return Ok(Self::Unix(PathBuf::from(path)));
        }
        if let Some(value) = value.strip_prefix("tcp://") {
            return Self::parse_tcp(value);
        }
        if value.contains("://") {
            return Err(RemotePlayerError::endpoint(format!(
                "daemon endpoint scheme is not supported yet: {value} (use local, unix:///path, tcp://127.0.0.1:port, or a plain socket path)"
            )));
        }
        Ok(Self::Unix(PathBuf::from(value)))
    }

    fn parse_tcp(value: &str) -> Result<Self, crate::RemotePlayerError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(RemotePlayerError::endpoint(
                "daemon endpoint tcp:// requires a host and port",
            ));
        }

        let (host, port) = value.rsplit_once(':').ok_or_else(|| {
            RemotePlayerError::endpoint(format!(
                "daemon endpoint tcp:// requires host:port: {value}"
            ))
        })?;

        let port: u16 = port.parse().map_err(|e| {
            RemotePlayerError::endpoint(format!(
                "daemon endpoint tcp:// requires a numeric port: {value}: {e}"
            ))
        })?;

        let ip = if host.eq_ignore_ascii_case("localhost") {
            Ipv4Addr::LOCALHOST
        } else {
            host.parse().map_err(|e| {
                RemotePlayerError::endpoint(format!(
                    "daemon endpoint tcp:// requires an IPv4 host: {value}: {e}"
                ))
            })?
        };

        Ok(Self::Tcp(SocketAddr::from((ip, port))))
    }

    pub(crate) fn connect_stream(&self) -> Result<SocketStream, crate::RemotePlayerError> {
        match self {
            Self::Local => {
                let path = PathBuf::from(mbv_config::control_socket_path());
                let start = std::time::Instant::now();
                loop {
                    match UnixStream::connect(&path) {
                        Ok(stream) => return Ok(SocketStream::Unix(stream)),
                        Err(e) if start.elapsed() >= LOCAL_DAEMON_CONNECT_RETRY_TIMEOUT => {
                            return Err(RemotePlayerError::connection(format!(
                                "cannot connect to daemon endpoint {self}: {e}"
                            )));
                        }
                        Err(_) => std::thread::sleep(LOCAL_DAEMON_CONNECT_RETRY_INTERVAL),
                    }
                }
            }
            Self::Unix(path) => UnixStream::connect(path)
                .map(SocketStream::Unix)
                .map_err(|e| {
                    RemotePlayerError::connection(format!(
                        "cannot connect to daemon endpoint {self}: {e}"
                    ))
                }),
            Self::Tcp(addr) => TcpStream::connect_timeout(addr, DAEMON_TCP_CONNECT_TIMEOUT)
                .map(SocketStream::Tcp)
                .map_err(|e| {
                    RemotePlayerError::connection(format!(
                        "cannot connect to daemon endpoint {self}: {e}"
                    ))
                }),
        }
    }

    /// Whether this endpoint is the same-machine daemon. Callers use this to
    /// decide connection behavior (e.g. `App::new_remote`'s `is_local_daemon`)
    /// so that distinction is derived from the endpoint itself rather than
    /// tracked separately and passed around as a disconnected bool.
    #[must_use]
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

impl std::fmt::Display for DaemonEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local ({})", mbv_config::control_socket_path()),
            Self::Unix(path) => write!(f, "unix://{}", path.display()),
            Self::Tcp(addr) => write!(f, "tcp://{addr}"),
        }
    }
}
