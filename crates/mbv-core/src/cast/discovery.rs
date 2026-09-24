// Google Cast receiver discovery over mDNS (`_googlecast._tcp`). Browsing is
// on-demand and bounded: callers ask for a snapshot, there is no background
// scan (see cast-device-discovery spec).

use std::time::{Duration, Instant};

use mdns_sd::{Receiver, ServiceDaemon, ServiceEvent};

const SERVICE_TYPE: &str = "_googlecast._tcp.local.";

/// A Google Cast receiver found on the LAN. `id` is the stable identifier the
/// receiver advertises in its TXT record, independent of `host`/`port` which
/// change with DHCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastReceiver {
    pub id: String,
    pub friendly_name: String,
    pub host: String,
    pub port: u16,
}

/// Browses for cast receivers for up to `timeout`. Never returns an error:
/// discovery failure is isolated from the panel (cast-device-discovery spec),
/// so any daemon/browse failure is logged and yields an empty list.
pub fn browse_cast_receivers(timeout: Duration) -> Vec<CastReceiver> {
    browse_cast_receivers_with(timeout, start_real_browse)
}

/// Re-runs discovery and resolves a previously persisted receiver by its
/// stable identifier. Returns `None` (unavailable) rather than a stale
/// address when the receiver does not appear in the fresh browse.
pub fn resolve_cast_receiver(id: &str, timeout: Duration) -> Option<CastReceiver> {
    find_by_id(browse_cast_receivers(timeout), id)
}

fn find_by_id(receivers: Vec<CastReceiver>, id: &str) -> Option<CastReceiver> {
    receivers.into_iter().find(|r| r.id == id)
}

fn start_real_browse() -> Result<(ServiceDaemon, Receiver<ServiceEvent>), String> {
    let daemon = ServiceDaemon::new().map_err(|e| e.to_string())?;
    let rx = daemon.browse(SERVICE_TYPE).map_err(|e| e.to_string())?;
    Ok((daemon, rx))
}

fn browse_cast_receivers_with(
    timeout: Duration,
    start: impl FnOnce() -> Result<(ServiceDaemon, Receiver<ServiceEvent>), String>,
) -> Vec<CastReceiver> {
    match start() {
        Ok((daemon, rx)) => {
            let receivers = collect_receivers(&rx, timeout);
            let _ = daemon.stop_browse(SERVICE_TYPE);
            let _ = daemon.shutdown();
            receivers
        }
        Err(e) => {
            log::warn!(target: "cast", "cast discovery browse failed to start: {e}");
            Vec::new()
        }
    }
}

fn collect_receivers(rx: &Receiver<ServiceEvent>, timeout: Duration) -> Vec<CastReceiver> {
    let deadline = Instant::now() + timeout;
    let mut receivers: std::collections::HashMap<String, CastReceiver> =
        std::collections::HashMap::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(remaining) {
            Ok(ServiceEvent::ServiceResolved(resolved)) => {
                if let Some(receiver) = cast_receiver_from_resolved(&resolved) {
                    receivers.insert(receiver.id.clone(), receiver);
                }
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    receivers.into_values().collect()
}

fn cast_receiver_from_resolved(resolved: &mdns_sd::ResolvedService) -> Option<CastReceiver> {
    let id = resolved
        .txt_properties
        .get_property_val_str("id")?
        .to_string();
    let friendly_name = resolved
        .txt_properties
        .get_property_val_str("fn")
        .unwrap_or(&resolved.fullname)
        .to_string();
    let address = resolved
        .addresses
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| resolved.addresses.iter().next())?;
    Some(CastReceiver {
        id,
        friendly_name,
        host: address.to_ip_addr().to_string(),
        port: resolved.port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browse_yielding_nothing_returns_empty_list() {
        let (tx, rx) = flume::unbounded();
        tx.send(ServiceEvent::SearchStarted(
            "_googlecast._tcp.local.".to_string(),
        ))
        .unwrap();
        drop(tx);
        let receivers = collect_receivers(&rx, Duration::from_millis(50));
        assert!(receivers.is_empty());
    }

    #[test]
    fn browse_start_failure_returns_empty_list_and_is_non_fatal() {
        let receivers = browse_cast_receivers_with(Duration::from_millis(10), || {
            Err("simulated failure".to_string())
        });
        assert!(receivers.is_empty());
    }

    #[test]
    fn resolve_by_identifier_absent_yields_none() {
        let receivers = vec![CastReceiver {
            id: "other-device".to_string(),
            friendly_name: "Other".to_string(),
            host: "192.168.0.50".to_string(),
            port: 8009,
        }];
        assert_eq!(find_by_id(receivers, "missing-device"), None);
    }

    #[test]
    fn resolve_by_identifier_present_returns_it() {
        let receivers = vec![CastReceiver {
            id: "device-1".to_string(),
            friendly_name: "Living Room".to_string(),
            host: "192.168.0.50".to_string(),
            port: 8009,
        }];
        let found = find_by_id(receivers, "device-1").unwrap();
        assert_eq!(found.friendly_name, "Living Room");
    }
}
