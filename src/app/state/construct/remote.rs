use super::{
    detached_socket_rx, independent_audiobookshelf_runtime, independent_emby_runtime, App,
};
use crate::app::state::bootstrap::{bootstrap_legacy_queue, LocalDaemonBootstrap};
use crate::app::state::types::playback::QueueScope;
use crate::app::state::types::player_tab::PlayerTab;
use crate::app::{bootstrap_unified_queue, AppInit, SessionEvent};
use mbv_core::api::{EmbyClient, EmbyItem};
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_core::remote_player::DaemonEndpoint;
use mbv_core::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use std::sync::{mpsc, Arc, Mutex};

/// Start MPRIS against the daemon's `RemotePlayer` (#175, previously done in
/// `main.rs::run_remote_app` before the constructor ran). Moved here so App
/// owns the resulting handle and can `rebind` it later if
/// `switch_to_direct_remote` / `restore_local_mode` swap which target owns
/// playback.
///
/// Test builds leave `mpris` unset (build() initializes it to None):
/// `mpris::start` claims `org.mpris.MediaPlayer2.mbv` on the real D-Bus
/// session bus from a thread with no shutdown path -- leaked into every test
/// process that constructs a remote App, where process teardown races it
/// (issue #757). Tests that exercise rebind inject `mpris::test_handle`
/// themselves.
#[cfg(not(test))]
fn start_mpris(remote: &mbv_core::remote_player::RemotePlayer) -> crate::mpris::MprisHandle {
    let mpris_remote = remote.clone();
    crate::mpris::start(
        std::sync::Arc::clone(&mpris_remote.status),
        move |cmd| {
            let _ = mpris_remote.send_command(cmd);
        },
        Some(remote.disconnected_flag()),
    )
}

/// The daemon-side queue snapshot an attaching session seeds its queue
/// scope, local-daemon bootstrap, and queue tabs from.
struct RemoteSnapshot {
    items: Vec<mbv_core::api::EmbyItem>,
    cursor: usize,
    unified_state: Option<mbv_core::ctrl::UnifiedQueueStateData>,
}

impl RemoteSnapshot {
    fn take(remote: &mbv_core::remote_player::RemotePlayer) -> Self {
        Self {
            items: remote.items.lock().unwrap().clone(),
            cursor: remote.status.lock().unwrap().current_idx,
            unified_state: remote.unified_queue_state(),
        }
    }

    /// Whether the daemon side has any queue content to present.
    fn has_items(&self) -> bool {
        self.unified_state
            .as_ref()
            .map_or(!self.items.is_empty(), |state| !state.slots.is_empty())
    }

    /// The queue scope the session opens in: remote when a network daemon
    /// already plays something, local otherwise.
    fn scope(&self, is_local: bool) -> QueueScope {
        if !is_local && self.has_items() {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    /// The legacy/unified bootstrap a local-daemon attach replays through
    /// `App::build`.
    fn local_bootstrap(&self, source: &crate::config::QueueSource) -> LocalDaemonBootstrap {
        self.unified_state.as_ref().map_or_else(
            || bootstrap_legacy_queue(self.items.clone(), self.cursor, source.clone()),
            bootstrap_unified_queue,
        )
    }

    /// The queue tabs the session mounts: one unified tab for a local
    /// daemon, separate local/remote tabs for a network daemon.
    fn tabs(
        self,
        is_local: bool,
        local_daemon_bootstrap: Option<&LocalDaemonBootstrap>,
    ) -> (PlayerTab, Option<PlayerTab>) {
        if is_local {
            // Local daemon: one unified queue, exactly like plain local
            // playback — no separate remote_player_tab, no scope pill.
            (
                local_daemon_bootstrap.as_ref().unwrap().player_tab.clone(),
                None,
            )
        } else {
            // Remote/network daemon: keep a separate remote queue so the
            // user can browse locally while the daemon plays elsewhere.
            (
                PlayerTab::default(),
                Some(self.unified_state.as_ref().map_or_else(
                    || PlayerTab::from_emby_items(self.items, self.cursor),
                    PlayerTab::from_unified_state,
                )),
            )
        }
    }
}

/// The service runtimes and Audiobookshelf credential state a remote or
/// attaching session starts from: the Emby runtime is ready when a concrete
/// client exists, idle otherwise; the Audiobookshelf startup request is
/// armed after `build`, once the runtime's generation exists.
struct RemoteServices {
    emby_runtime: EmbyRuntime,
    audiobookshelf_runtime: AudiobookshelfRuntime,
    audiobookshelf_configured: bool,
    audiobookshelf_credential_present: bool,
}

fn remote_services(
    app_config: &crate::config::Config,
    client_arc: Option<&Arc<Mutex<EmbyClient>>>,
) -> RemoteServices {
    let emby_configured = app_config.emby_setup.is_some();
    let emby_credential_present =
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby).is_some();
    let audiobookshelf_configured = app_config.audiobookshelf_setup.is_some();
    let audiobookshelf_credential_present =
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
            .is_some();
    let emby_runtime = client_arc.map_or_else(
        || independent_emby_runtime(emby_configured, emby_credential_present),
        |client| EmbyRuntime::ready(std::sync::Arc::clone(client)),
    );
    let audiobookshelf_runtime = independent_audiobookshelf_runtime(
        audiobookshelf_configured,
        audiobookshelf_credential_present,
    );
    RemoteServices {
        emby_runtime,
        audiobookshelf_runtime,
        audiobookshelf_configured,
        audiobookshelf_credential_present,
    }
}

impl App {
    /// `endpoint` is the daemon endpoint the remote player is connected to.
    /// The endpoint's `is_local()` distinguishes local-daemon attach
    /// (`DaemonEndpoint::Local`) from a genuinely remote daemon:
    /// - `Local`: behaves like a plain local session — one unified queue,
    ///   normal queue-state persistence — the only difference is that the
    ///   daemon owns mpv instead of an in-process `Player`.
    /// - `Tcp`/`Unix`: a separate `remote_player_tab` is kept so the user
    ///   can browse locally while a daemon elsewhere plays something else,
    ///   with the Local/Remote scope pill to switch between them.
    #[cfg(test)]
    pub fn new_remote(
        client: EmbyClient,
        remote: mbv_core::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &DaemonEndpoint,
    ) -> Self {
        let config = crate::config::load_config().unwrap_or_default();
        Self::new_remote_optional_with_config(Some(client), remote, player_rx, endpoint, config)
    }

    #[cfg(test)]
    pub fn new_remote_with_config(
        client: EmbyClient,
        remote: mbv_core::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &DaemonEndpoint,
        config: crate::config::Config,
    ) -> Self {
        Self::new_remote_optional_with_config(Some(client), remote, player_rx, endpoint, config)
    }

    pub fn new_remote_optional_with_config(
        client: Option<EmbyClient>,
        remote: mbv_core::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &DaemonEndpoint,
        app_config: crate::config::Config,
    ) -> Self {
        let (_, ws_rx) = mpsc::channel::<mbv_ws::WsEvent>();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel::<SessionEvent>();
        let (card_image_tx, card_image_rx) =
            mpsc::channel::<(String, Option<image::DynamicImage>)>();
        let (notif_action_tx, notif_action_rx) = mpsc::channel::<String>();
        let (search_tx, search_rx) = mpsc::channel::<(String, Result<Vec<EmbyItem>, String>)>();
        let ui_config = crate::config::load_ui_config().unwrap_or_default();
        let hidden_libraries = app_config.hidden_libraries.clone();
        let library_routes = app_config.library_routes.clone();
        let music_levels = app_config.music_levels.clone();
        let always_play_next = app_config.always_play_next;
        // Both side effects below touch real system state, so test builds
        // must never run them (issue #757): the eviction thread scans and
        // prunes the user's real image-cache dir, and `mpris::start` claims
        // `org.mpris.MediaPlayer2.mbv` on the real D-Bus session bus from a
        // thread that has no shutdown path -- leaked into every test process
        // that constructs a remote App, where process teardown races it.
        #[cfg(not(test))]
        crate::config::evict_old_image_cache();
        // EmbyClient retains this snapshot only for constructing Emby API
        // requests. App general state owns the independent application copy;
        // it is never synchronized back into this concrete API boundary.
        let client_arc = client.map(|client| Arc::new(Mutex::new(client)));
        let services = remote_services(&app_config, client_arc.as_ref());
        let config = Arc::new(Mutex::new(app_config));
        let remote_queue_source = remote.queue_source.lock().unwrap().clone();
        let snapshot = RemoteSnapshot::take(&remote);
        let initial_queue_scope = snapshot.scope(endpoint.is_local());
        let local_daemon_bootstrap = endpoint
            .is_local()
            .then(|| snapshot.local_bootstrap(&remote_queue_source));
        #[cfg(not(test))]
        let mpris_handle = Some(start_mpris(&remote));
        #[cfg(test)]
        let mpris_handle = None;
        let player = PlayerProxy::remote(remote, always_play_next);
        let (player_tab, remote_player_tab) =
            snapshot.tabs(endpoint.is_local(), local_daemon_bootstrap.as_ref());
        let mut app = Self::build(AppInit {
            config,
            emby_runtime: services.emby_runtime,
            audiobookshelf_runtime: services.audiobookshelf_runtime,
            emby_startup_rx: None,
            emby_startup_request: None,
            audiobookshelf_startup_rx: None,
            audiobookshelf_startup_request: None,
            audiobookshelf_test_rx: None,
            audiobookshelf_setup_rx: None,
            emby_setup_form: None,
            emby_setup_rx: None,
            player,
            player_rx,
            ws_rx,
            ws_send_tx: None,
            audiobookshelf_socket_rx: detached_socket_rx(),
            audiobookshelf_socket_tx: None,
            audiobookshelf_socket_generation: None,
            player_tab,
            remote_player_tab,
            initial_queue_scope,
            system_notifications: false,
            image_protocol: ui_config.image_protocol.clone(),
            image_protocol_enabled: ui_config.image_protocol.is_some(),
            hidden_libraries,
            library_routes,
            music_levels,
            use_nerd_fonts: ui_config.use_nerd_fonts,
            indicator_style: ui_config.indicator_style.parse().unwrap_or_default(),
            image_cache_size: ui_config.image_cache_size,
            visualizer_glyph: ui_config.visualizer_glyph,
            lib_tx,
            lib_rx,
            sessions_tx,
            sessions_rx,
            card_image_tx,
            card_image_rx,
            notif_action_tx,
            notif_action_rx,
            search_tx,
            search_rx,
            idle_feed: None,
        });
        app.mpris = mpris_handle;
        app.player_endpoint = Some(endpoint.clone());
        app.home_is_local_daemon = endpoint.is_local();
        app.sync_subtitle_prefs_to_player();
        app.launched_as_remote = true;
        debug_assert_eq!(
            app.player.is_remote(),
            app.player_endpoint.is_some(),
            "player-endpoint invariant"
        );
        if endpoint.is_local() {
            let bootstrap = local_daemon_bootstrap.unwrap();
            app.queue_source = bootstrap.queue_source;
            app.last_played_item_id = bootstrap.last_played_item_id;
            app.last_played_completed = bootstrap.last_played_completed;
            app.try_auto_reconnect();
        } else {
            app.queue_source = remote_queue_source;
        }
        app.audiobookshelf_startup_request = (services.audiobookshelf_configured
            && services.audiobookshelf_credential_present)
            .then_some((
                app.config.lock().unwrap().clone(),
                app.audiobookshelf_runtime.generation(),
            ));
        app
    }
}
