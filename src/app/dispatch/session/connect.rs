use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, PlayerTab};
use mbv_ctrl::player::PlayerEvent;
use mbv_emby::parse_mbv_direct_tcp_port;
use mbv_player::PlayerProxy;
use std::sync::mpsc;
use std::time::{Duration, Instant};

impl App {
    pub(in crate::app) fn session_direct_endpoint(
        &self,
        sess: &mbv_emby::SessionInfo,
    ) -> Option<mbv_remote_player::DaemonEndpoint> {
        if !sess.client.eq_ignore_ascii_case("mbv") {
            return None;
        }
        if let Some(port) = parse_mbv_direct_tcp_port(&sess.supported_commands) {
            if let Ok(ip) = sess.host.parse::<std::net::Ipv4Addr>() {
                return Some(mbv_remote_player::DaemonEndpoint::Tcp(
                    std::net::SocketAddr::from((ip, port)),
                ));
            }
            tracing::warn!(name: "sessions.direct_endpoint.invalid_host", target: "sessions", device = %sess.device_name, port, host = %sess.host, "session advertised direct TCP port with invalid host");
        }
        let client = self.emby_client()?;
        let client = client.lock().unwrap();
        sess.device_name
            .eq_ignore_ascii_case(&client.device_name)
            .then_some(mbv_remote_player::DaemonEndpoint::Local)
    }

    /// Blocking `GET /Sessions` (unfiltered), factored out only so tests
    /// can override it via `SESSIONS_LOAD_OVERRIDE` -- mirrors
    /// `connect_daemon_route_endpoint`'s `#[cfg(test)]` seam. Callers:
    /// `try_auto_reconnect`'s `DirectSession` case (#236) and the F2
    /// "Library Routes" device picker (`enter_device_stage`, #256) --
    /// library-route *resolution* itself no longer calls this (#256).
    pub(in crate::app) fn fetch_sessions_blocking(
        &self,
    ) -> Result<Vec<mbv_emby::SessionInfo>, mbv_emby::EmbyError> {
        #[cfg(test)]
        if let Some(f) = *crate::app::SESSIONS_LOAD_OVERRIDE.lock().unwrap() {
            let Some(client) = self.emby_client() else {
                return Err(mbv_emby::EmbyError::from(
                    mbv_emby::EmbyFailure::unavailable("Emby is unavailable"),
                ));
            };
            return f(&client.lock().unwrap());
        }
        let Some(client) = self.emby_client() else {
            return Err(mbv_emby::EmbyError::from(
                mbv_emby::EmbyFailure::unavailable("Emby is unavailable"),
            ));
        };

        client.lock().unwrap().get_sessions_unfiltered()
    }

    pub(in crate::app) fn connect_direct_endpoint(
        endpoint: &mbv_remote_player::DaemonEndpoint,
    ) -> Result<
        (mbv_remote_player::RemotePlayer, mpsc::Receiver<PlayerEvent>),
        mbv_remote_player::RemotePlayerError,
    > {
        #[cfg(test)]
        if let Some(connect) = *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() {
            return connect(endpoint);
        }

        mbv_remote_player::RemotePlayer::connect_endpoint(endpoint)
    }

    /// Lazy, on-demand connect to a daemon route endpoint (issue #222's
    /// lifecycle primitive). Unlike `connect_direct_endpoint` (Sessions-panel
    /// "Direct Remote" upgrade, keyed off a discovered `SessionInfo`), this
    /// targets a statically configured `DaemonEndpoint` with no session
    /// discovery involved -- the shape #223's per-library routing needs.
    ///
    /// Under multi-connection (v4), connecting does NOT evict other ctrl
    /// clients. Authority is determined by command flow, not connection
    /// lifecycle (ADR 0014 supersedes ADR 0003).
    ///
    fn connect_daemon_route_endpoint(
        endpoint: &mbv_remote_player::DaemonEndpoint,
    ) -> Result<
        (mbv_remote_player::RemotePlayer, mpsc::Receiver<PlayerEvent>),
        mbv_remote_player::RemotePlayerError,
    > {
        #[cfg(test)]
        if let Some(connect) = *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() {
            return connect(endpoint).into_result();
        }

        tracing::info!(name: "daemon_route.connect.started", target: "daemon_route", endpoint = %endpoint, "connecting to daemon route; existing clients are retained");
        mbv_remote_player::RemotePlayer::connect_endpoint(endpoint)
    }

    /// Attempts a lazy connect to `endpoint` for the route named
    /// `route_label` (e.g. a library name from #239's `library_routes`, or a
    /// generic label for the wildcard "route everything" case). On success,
    /// returns `Ok` with the connected `RemotePlayer` and its event receiver
    /// for the caller to swap in (mirroring `switch_to_direct_remote`'s
    /// shape). On failure, per #222: falls back to (stays on) local
    /// playback and schedules no retry -- but this primitive does NOT flash
    /// the warning itself. It logs the raw connect error internally
    /// (`target: "daemon_route"`), then returns `Err(message)` where
    /// `message` is the fully-formatted, ready-to-display status-bar
    /// warning text. Flashing is left to the caller deliberately: #223's
    /// per-library swap function needs to choose *how* to fall back --
    /// `flash(message, ToastSeverity::Warning)` directly when it was already local, or
    /// threading `message` through a `restore_local_mode`-style teardown
    /// when swapping away from a previously active *different* route -- and
    /// having this primitive flash unconditionally would risk a second,
    /// conflicting flash on top of that teardown path's own flash. The
    /// caller is expected to try again only on its own next natural trigger
    /// (e.g. the next play/enqueue into this route), never from a
    pub(in crate::app) fn try_daemon_route_connect(
        endpoint: &mbv_remote_player::DaemonEndpoint,
        route_label: &str,
    ) -> Result<
        (mbv_remote_player::RemotePlayer, mpsc::Receiver<PlayerEvent>),
        mbv_remote_player::RemotePlayerError,
    > {
        tracing::info!(name: "daemon_route.connect.started", target: "daemon_route", route = %route_label, endpoint = %endpoint, "daemon route connection started");
        Self::connect_daemon_route_endpoint(endpoint)
            .inspect(|_| {
                tracing::info!(name: "daemon_route.connect.succeeded", target: "daemon_route", route = %route_label, endpoint = %endpoint, "daemon route connection succeeded");
            })
            .inspect_err(|error| {
                tracing::warn!(name: "daemon_route.connect.failed", target: "daemon_route", route = %route_label, endpoint = %endpoint, error = %error, "daemon route connection failed");
            })
    }

    /// Reattach to the same daemon endpoint after an unannounced drop when
    /// `auto_reconnect` is enabled. Runs before the local-restore fallback so
    /// a restarted daemon lands the TUI straight back on its canonical queue
    /// instead of dropping to local playback for the rest of the session.
    ///
    /// Returns `true` when the player has been swapped to a live reattach and
    /// the caller must skip `restore_local_mode`. Local daemons are excluded:
    /// those already have the modal / `home_is_local_daemon` reconnect paths.
    pub(in crate::app) fn try_reattach_remote_daemon(&mut self) -> bool {
        self.try_reattach_remote_daemon_with_sleep(std::thread::sleep)
    }

    /// Retry loop with an injectable sleep clock so tests can observe the
    /// backoff sequence without paying wall-clock time. The loop, attempt
    /// count, and backoff constants are identical to the shipped path — only
    /// the clock differs.
    pub(in crate::app) fn try_reattach_remote_daemon_with_sleep(
        &mut self,
        sleep: impl Fn(Duration),
    ) -> bool {
        let Some(endpoint) = self.player_endpoint.clone() else {
            return false;
        };
        if matches!(endpoint, mbv_remote_player::DaemonEndpoint::Local) {
            return false;
        }
        if !self.config.lock().unwrap().auto_reconnect {
            return false;
        }
        tracing::info!(name: "auto_reconnect.reattach.started", target: "auto_reconnect", endpoint = %endpoint, "reattaching to daemon");
        // The daemon may still be coming back up, so retry a few times with
        // backoff before falling through to the local-restore path. Bounded
        // and short so an unreachable daemon cannot wedge the UI.
        let mut backoff = Duration::from_millis(300);
        for attempt in 0..3 {
            match Self::connect_daemon_route_endpoint(&endpoint) {
                Ok((remote, remote_rx)) => {
                    self.attach_reattached_daemon(remote, remote_rx, &endpoint, attempt);
                    return true;
                }
                Err(e) => {
                    tracing::warn!(name: "auto_reconnect.reattach.failed", target: "auto_reconnect", attempt, endpoint = %endpoint, error = %e, "reattach attempt failed");
                    sleep(backoff);
                    backoff *= 2;
                }
            }
        }
        false
    }

    fn attach_reattached_daemon(
        &mut self,
        remote: mbv_remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &mbv_remote_player::DaemonEndpoint,
        attempt: usize,
    ) {
        let initial_unified_state = remote.unified_queue_state();
        let always_play_next = self.config.lock().unwrap().always_play_next;
        let mpris_remote = remote.clone();
        // #233: tear down the dead connection before replacing it so its
        // reader thread observes the shutdown and exits instead of leaking.
        self.player.disconnect_remote();
        self.player = PlayerProxy::remote(remote, always_play_next);
        self.player_rx = remote_rx;
        self.player_endpoint = Some(endpoint.clone());
        debug_assert_eq!(self.player.is_remote(), self.player_endpoint.is_some());
        if let Some(handle) = &self.mpris {
            let disconnected = mpris_remote.disconnected_flag();
            mbv_desktop::mpris::rebind(
                handle,
                std::sync::Arc::clone(&mpris_remote.status),
                move |transport| mpris_remote.send_transport(transport),
                Some(disconnected),
            );
        }
        let mut tab = initial_unified_state
            .as_ref()
            .map_or_else(PlayerTab::default, PlayerTab::from_unified_state);
        if let Some(previous_tab) = &self.remote_player_tab {
            tab.adopt_revision_mint(previous_tab.revision_mint());
        }
        self.remote_player_tab = Some(tab);
        self.remote.direct_remote_connected = true;
        self.advance_queue_epoch();
        self.remote.session_miss_count = 0;
        self.remote.remote_pos_s = 0;
        self.remote.remote_pos_at = Instant::now();
        self.remote.remote_api_pos_advanced_at = Instant::now()
            .checked_sub(Duration::from_secs(60))
            .unwrap_or_else(Instant::now);
        self.remote.remote_seek_pending_until = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        self.remote.runtime_zero_since = None;
        self.next_up_item = None;
        self.display_peer_queue_on_connect();
        self.sync_subtitle_prefs_to_player();
        self.flash(
            format!("Reconnected to daemon (attempt {})", attempt + 1),
            ToastSeverity::Success,
        );
    }

    /// Restores the remote connection active when mbv last exited (issue
    /// #236 -- #222's original "auto-reconnect" intent). Called once per
    /// launch: synchronously from `App::new_remote`'s local-daemon-attach
    /// path (construct.rs) when the Emby client is already available at
    /// construction, or from `apply_emby_completion`
    /// (`app_emby_service_completion.rs`) once the async Emby startup used by
    /// `App::new_independent` completes. A genuinely remote
    /// `--connect-daemon` launch is a separate, unaffected mechanism per
    /// ADR 0010. A no-op unless `auto_reconnect` is enabled and
    /// `load_last_remote_connection` has a record. One shot, no retry: a
    /// failed connect, a route no longer present in `library_routes`, or a
    /// device not found in the current session list all fall back to (stay
    /// on) local playback, exactly like #222's per-play lazy-connect
    /// fallback rule -- never a hard failure at startup.
    pub(in crate::app) fn try_auto_reconnect(&mut self) {
        if !self.config.lock().unwrap().auto_reconnect {
            tracing::info!(name: "auto_reconnect.reconnect.disabled", target: "auto_reconnect", "auto-reconnect disabled; staying local");
            return;
        }
        tracing::info!(name: "auto_reconnect.state_load.started", target: "auto_reconnect", "loading auto-reconnect state");
        let last = match mbv_config::load_last_remote_connection() {
            Ok(Some(last)) => last,
            Ok(None) => {
                tracing::info!(name: "auto_reconnect.state.missing", target: "auto_reconnect", "auto-reconnect state missing; staying local");
                return;
            }
            Err(e) => {
                tracing::warn!(name: "auto_reconnect.state_load.failed", target: "auto_reconnect", error = %e, "auto-reconnect state load failed; staying local");
                return;
            }
        };
        match last {
            mbv_config::LastRemoteConnection::LibraryRoute { library } => {
                tracing::info!(name: "auto_reconnect.state.loaded", target: "auto_reconnect", connection_type = "library_route", library = %library, "auto-reconnect state loaded");
                let Some((name, endpoint)) = self.resolve_route_for_library(&library) else {
                    tracing::info!(name: "auto_reconnect.library_route.unresolved", target: "auto_reconnect", library = %library, "persisted library route no longer resolves; staying local");
                    return;
                };
                match Self::try_daemon_route_connect(&endpoint, &name) {
                    Ok((remote, remote_rx)) => {
                        self.switch_to_library_route(&name, remote, remote_rx, &endpoint);
                    }
                    Err(_) => self.flash(
                        format!(
                            "\u{26a0} {name} route unreachable, using local playback (mbv.log)"
                        ),
                        ToastSeverity::Warning,
                    ),
                }
            }
            mbv_config::LastRemoteConnection::DirectSession { device_name } => {
                tracing::info!(name: "auto_reconnect.state.loaded", target: "auto_reconnect", connection_type = "direct_session", device = %device_name, "auto-reconnect state loaded");
                let sessions = match self.fetch_sessions_blocking() {
                    Ok(sessions) => sessions,
                    Err(e) => {
                        tracing::warn!(name: "auto_reconnect.sessions_load.failed", target: "auto_reconnect", error = %e, "failed to list sessions");
                        self.flash(format!(
                            "\u{26a0} Auto-reconnect couldn't list sessions ({e}), using local playback"
                        ), ToastSeverity::Warning);
                        return;
                    }
                };
                if let Some(sess) = sessions
                    .into_iter()
                    .find(|s| s.device_name.eq_ignore_ascii_case(&device_name))
                {
                    tracing::info!(name: "auto_reconnect.direct_session.resolved", target: "auto_reconnect", device = %device_name, session = %sess.id, "direct session resolved; connecting");
                    self.connect_to_session(&sess);
                    if self.remote.direct_remote_connected {
                        tracing::info!(name: "auto_reconnect.direct_session.connected", target: "auto_reconnect", device = %device_name, outcome = "direct_daemon_upgrade", "direct session connected");
                    } else if self.connected_session_id.is_some() {
                        tracing::info!(name: "auto_reconnect.direct_session.connected", target: "auto_reconnect", device = %device_name, outcome = "emby_session_control", "direct session connected");
                    } else {
                        tracing::warn!(name: "auto_reconnect.direct_session.connect_failed", target: "auto_reconnect", device = %device_name, "direct session connection failed; staying local");
                    }
                } else {
                    tracing::info!(name: "auto_reconnect.direct_session.not_found", target: "auto_reconnect", device = %device_name, "device not found in current sessions; staying local");
                    self.flash(
                        format!("\u{26a0} {device_name} not found, using local playback"),
                        ToastSeverity::Warning,
                    );
                }
            }
        }
    }
}
