use super::*;
use std::net::SocketAddr;
use std::path::PathBuf;

#[test]
fn daemon_endpoint_parses_local_and_unix_paths() {
    assert_eq!(
        DaemonEndpoint::parse("local").unwrap(),
        DaemonEndpoint::Local
    );
    assert_eq!(DaemonEndpoint::parse("").unwrap(), DaemonEndpoint::Local);
    assert_eq!(
        DaemonEndpoint::parse("unix:///tmp/mbv.sock").unwrap(),
        DaemonEndpoint::Unix(PathBuf::from("/tmp/mbv.sock"))
    );
    assert_eq!(
        DaemonEndpoint::parse("/tmp/mbv.sock").unwrap(),
        DaemonEndpoint::Unix(PathBuf::from("/tmp/mbv.sock"))
    );
    assert_eq!(
        DaemonEndpoint::parse("tcp://localhost:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], 1234)))
    );
    assert_eq!(
        DaemonEndpoint::parse("tcp://127.0.0.1:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], 1234)))
    );
    assert_eq!(
        DaemonEndpoint::parse("tcp://127.0.0.2:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 2], 1234)))
    );
}

#[test]
fn daemon_endpoint_rejects_unsupported_schemes() {
    assert_eq!(
        DaemonEndpoint::parse("tcp://10.0.0.1:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([10, 0, 0, 1], 1234)))
    );
    DaemonEndpoint::parse("tcp://[::1]:4321").unwrap_err();
    DaemonEndpoint::parse("unix://").unwrap_err();
    DaemonEndpoint::parse("http://localhost:1234").unwrap_err();
}

#[test]
fn resolve_library_route_has_no_wildcard_fallback() {
    let mut routes = std::collections::BTreeMap::new();
    routes.insert("music".to_string(), "tcp://192.168.0.104:47788".to_string());
    assert_eq!(
        resolve_library_route(&routes, "Music"),
        Some(DaemonEndpoint::Tcp("192.168.0.104:47788".parse().unwrap()))
    );
    assert_eq!(resolve_library_route(&routes, "movies"), None);
}

#[test]
fn resolve_library_route_rejects_a_bare_device_name_as_malformed() {
    // A stale pre-#256 config entry (device name, no scheme) must
    // NOT silently resolve -- DaemonEndpoint::parse would otherwise
    // accept it as a bogus Unix(PathBuf) socket path. Library routing
    // is tcp://-only (#239 addendum), so anything that doesn't parse
    // to Tcp(_) is treated as malformed: logged and skipped.
    let mut routes = std::collections::BTreeMap::new();
    routes.insert("music".to_string(), "living-room-pc".to_string());
    assert_eq!(resolve_library_route(&routes, "music"), None);
}

#[test]
fn resolve_library_route_rejects_unix_and_local_endpoints() {
    // Library routing is remote-only -- a unix:// or bare "local"
    // value is well-formed as a DaemonEndpoint but not a valid
    // library route, so it must still resolve to None.
    let mut routes = std::collections::BTreeMap::new();
    routes.insert("music".to_string(), "unix:///run/mbvd.sock".to_string());
    routes.insert("movies".to_string(), "local".to_string());
    assert_eq!(resolve_library_route(&routes, "music"), None);
    assert_eq!(resolve_library_route(&routes, "movies"), None);
}
