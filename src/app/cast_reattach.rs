// Cast session lifecycle across mbv exit and relaunch (## 7): reattaching to
// a persisted receiver at launch (7.3, 7.4, 7.5) and the shared
// resolve-and-connect primitive (`connect_cast_receiver`) that both this
// stage and task 8.3's attach-on-selection reuse. Teardown-time persistence
// (7.1, 7.2) lives in `run_loop_events_teardown.rs` beside the
// `LastRemoteConnection` persistence it mirrors; disconnect presentation
// (7.6) lives in `cast_status_actions.rs`'s `apply_cast_status`.

use super::types_cast::{CastEvent, CastJob};
use super::App;
use std::sync::mpsc::Sender;
use std::time::Duration;

/// Bounded so a launch with `auto_reconnect` on but no receiver actually
/// reachable doesn't wait indefinitely -- this runs on a background thread
/// (see `connect_cast_receiver`), so it never blocks startup regardless.
const CAST_REATTACH_TIMEOUT: Duration = Duration::from_secs(3);

impl App {
    /// Restores control of a cast receiver mbv was attached to at exit
    /// (7.3), the cast counterpart to `try_auto_reconnect`. Called once per
    /// launch from `App::new_remote_optional_with_config`'s local-daemon
    /// path (already has everything it needs at construction) and from
    /// `apply_emby_completion` (once `App::new_independent`'s async Emby
    /// startup completes) -- unlike `try_auto_reconnect`, not gated on
    /// `endpoint.is_local()`, since cast discovery/connect run on this
    /// machine's own LAN regardless of which daemon the player talks to.
    /// A no-op unless `auto_reconnect` is enabled and a receiver id was
    /// persisted. Never dispatches or alters what the receiver plays: a
    /// successful connect only attaches (`CastEvent::Connected`); whatever
    /// the receiver already reports arrives through the ordinary status-poll
    /// cycle (task 6.1).
    pub(super) fn try_cast_auto_reconnect(&mut self) {
        if !self.config.lock().unwrap().auto_reconnect {
            log::info!(target: "auto_reconnect", "cast auto-reconnect disabled; not attaching");
            return;
        }
        let receiver_id = match mbv_core::config::load_last_cast_receiver() {
            Ok(Some(id)) => id,
            Ok(None) => {
                log::info!(target: "auto_reconnect", "no persisted cast receiver; not attaching");
                return;
            }
            Err(e) => {
                log::warn!(target: "auto_reconnect", "cast receiver state load failed: {e}");
                return;
            }
        };
        log::info!(target: "auto_reconnect", "attempting cast reattach receiver_id={receiver_id:?}");
        self.connect_cast_receiver(receiver_id, CAST_REATTACH_TIMEOUT);
    }

    /// Resolves `id`'s current address via a fresh discovery browse,
    /// connects, and hands the connected transport to a new cast worker
    /// (`types_cast::spawn_cast_worker`), reporting the outcome back over
    /// `cast_tx` as `CastEvent::Connected`/`ConnectFailed`. Runs entirely on
    /// a background thread: both the discovery browse and `CastClient::connect`
    /// are blocking network calls (mirrors `try_daemon_route_connect`'s
    /// blocking-network style), and neither may run on the caller's thread.
    ///
    /// Shared by 7.3's reattach above and task 8.3 (attach-on-selection from
    /// the discovery panel) -- reuse this rather than duplicating the
    /// resolve -> connect -> spawn_cast_worker -> report-back sequence.
    pub(super) fn connect_cast_receiver(&mut self, id: String, timeout: Duration) {
        let tx = self.cast_tx.clone();
        let connect = cast_connect_fn();
        std::thread::spawn(move || {
            let event = match connect(&id, timeout) {
                Ok(client) => CastEvent::Connected {
                    receiver_id: id,
                    client,
                },
                Err(error) => CastEvent::ConnectFailed {
                    receiver_id: id,
                    error,
                },
            };
            let _ = tx.send(event);
        });
    }
}

/// Resolves `id` by identifier via a fresh discovery browse -- `None` (no
/// address, no attempt to connect) is treated as unavailable (7.5) rather
/// than a stale address -- then connects and spawns the worker thread that
/// owns the resulting `CastClient` for the rest of this attachment's life.
/// `spawn_cast_worker`'s fallible `build` (see its doc comment) is what
/// lets `CastClient::connect`'s real failure mode surface here as an `Err`
/// instead of only being discoverable once a job is submitted.
fn resolve_and_connect_cast_receiver(
    id: &str,
    timeout: Duration,
) -> Result<Sender<CastJob>, String> {
    let receiver = mbv_core::cast_discovery::resolve_cast_receiver(id, timeout)
        .ok_or_else(|| "receiver not found".to_string())?;
    super::types_cast::spawn_cast_worker(move || {
        mbv_core::cast_client::CastClient::connect(&receiver.host, receiver.port)
    })
}

#[cfg(test)]
fn cast_connect_fn() -> super::CastConnectFn {
    (*super::CAST_CONNECT_OVERRIDE.lock().unwrap()).unwrap_or(resolve_and_connect_cast_receiver)
}

#[cfg(not(test))]
fn cast_connect_fn() -> fn(&str, Duration) -> Result<Sender<CastJob>, String> {
    resolve_and_connect_cast_receiver
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use crate::app::types_cast::{spawn_fake_cast_worker, FakeCastTransport};

    fn set_cast_connect_override(f: super::super::CastConnectFn) {
        *super::super::CAST_CONNECT_OVERRIDE.lock().unwrap() = Some(f);
    }

    fn clear_cast_connect_override() {
        *super::super::CAST_CONNECT_OVERRIDE.lock().unwrap() = None;
    }

    fn recv_cast_event(app: &App) -> CastEvent {
        app.cast_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("connect_cast_receiver should report an outcome")
    }

    #[test]
    fn try_cast_auto_reconnect_is_a_no_op_when_disabled() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _ = mbv_core::config::save_last_cast_receiver(Some("device-1"));
        let mut app = make_app_stub();
        assert!(!app.config.lock().unwrap().auto_reconnect);

        app.try_cast_auto_reconnect();

        assert!(!app.is_cast_attached());
        assert!(app.cast_rx.try_recv().is_err());
    }

    #[test]
    fn try_cast_auto_reconnect_is_a_no_op_when_nothing_was_persisted() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_app_stub();
        app.config.lock().unwrap().auto_reconnect = true;

        app.try_cast_auto_reconnect();

        assert!(!app.is_cast_attached());
        assert!(app.cast_rx.try_recv().is_err());
    }

    #[test]
    fn try_cast_auto_reconnect_attaches_without_dispatching_when_the_receiver_is_playing() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = super::super::CAST_CONNECT_TEST_LOCK.lock().unwrap();
        fn connect_ok(_id: &str, _timeout: Duration) -> Result<Sender<CastJob>, String> {
            let (tx, _calls) = spawn_fake_cast_worker(FakeCastTransport {
                status: Ok(mbv_core::cast_client::CastStatus {
                    position_seconds: Some(10.0),
                    duration_seconds: Some(100.0),
                    playback_rate: 1.0,
                    state: mbv_core::cast_client::CastPlaybackState::Playing,
                    playing_content_id: Some("https://a".to_string()),
                }),
                ..Default::default()
            });
            Ok(tx)
        }
        set_cast_connect_override(connect_ok);
        let _ = mbv_core::config::save_last_cast_receiver(Some("device-1"));
        let mut app = make_app_stub();
        app.config.lock().unwrap().auto_reconnect = true;

        app.try_cast_auto_reconnect();
        let event = recv_cast_event(&app);
        app.handle_cast_event(event);
        clear_cast_connect_override();

        assert!(app.is_cast_attached());
        assert_eq!(
            app.cast_attachment.as_ref().unwrap().receiver_id,
            "device-1"
        );
        assert!(app.cast_attachment.as_ref().unwrap().dispatched.is_empty());

        // The receiver's already-playing status arrives through the
        // ordinary status-poll cycle (task 6.1), not as part of reattach
        // itself -- exercise that here to confirm it still doesn't dispatch.
        app.spawn_cast_status_poll();
        let status_event = recv_cast_event(&app);
        app.handle_cast_event(status_event);
        assert!(app.cast_attachment.as_ref().unwrap().dispatched.is_empty());
        assert!(app
            .cast_attachment
            .as_ref()
            .unwrap()
            .status
            .as_ref()
            .is_some_and(|s| s.state == mbv_core::cast_client::CastPlaybackState::Playing));
    }

    #[test]
    fn try_cast_auto_reconnect_attaches_without_dispatching_when_the_receiver_is_idle() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = super::super::CAST_CONNECT_TEST_LOCK.lock().unwrap();
        fn connect_ok(_id: &str, _timeout: Duration) -> Result<Sender<CastJob>, String> {
            let (tx, _calls) = spawn_fake_cast_worker(FakeCastTransport {
                status: Ok(mbv_core::cast_client::CastStatus {
                    position_seconds: None,
                    duration_seconds: None,
                    playback_rate: 1.0,
                    state: mbv_core::cast_client::CastPlaybackState::Idle,
                    playing_content_id: None,
                }),
                ..Default::default()
            });
            Ok(tx)
        }
        set_cast_connect_override(connect_ok);
        let _ = mbv_core::config::save_last_cast_receiver(Some("device-1"));
        let mut app = make_app_stub();
        app.config.lock().unwrap().auto_reconnect = true;

        app.try_cast_auto_reconnect();
        let event = recv_cast_event(&app);
        app.handle_cast_event(event);
        clear_cast_connect_override();

        assert!(app.is_cast_attached());
        assert!(app.cast_attachment.as_ref().unwrap().dispatched.is_empty());

        app.spawn_cast_status_poll();
        let status_event = recv_cast_event(&app);
        app.handle_cast_event(status_event);
        assert!(app.cast_attachment.as_ref().unwrap().dispatched.is_empty());
        assert_eq!(
            app.cast_now_playing_title(app.cast_attachment.as_ref().unwrap()),
            None
        );
    }

    #[test]
    fn try_cast_auto_reconnect_presents_an_unavailable_receiver_without_attaching() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = super::super::CAST_CONNECT_TEST_LOCK.lock().unwrap();
        fn connect_not_found(_id: &str, _timeout: Duration) -> Result<Sender<CastJob>, String> {
            Err("receiver not found".to_string())
        }
        set_cast_connect_override(connect_not_found);
        let _ = mbv_core::config::save_last_cast_receiver(Some("device-1"));
        let mut app = make_app_stub();
        app.config.lock().unwrap().auto_reconnect = true;

        app.try_cast_auto_reconnect();
        let event = recv_cast_event(&app);
        app.handle_cast_event(event);
        clear_cast_connect_override();

        assert!(!app.is_cast_attached());
        assert!(app.status.contains("Couldn't connect to cast receiver"));
    }
}
