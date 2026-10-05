use super::{
    App, detached_socket_rx, independent_audiobookshelf_runtime, independent_emby_runtime,
};
use crate::app::state::bootstrap::{LocalDaemonBootstrap, bootstrap_legacy_queue};
use crate::app::state::queue_view::QueueView;
use crate::app::state::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use crate::app::{AppInit, bootstrap_unified_queue};
use mbv_ctrl::player::PlayerEvent;
use mbv_emby::EmbyClient;
use mbv_player::PlayerProxy;
use mbv_remote_player::DaemonEndpoint;
use mbv_ui_model::playback::QueueScope;
use std::sync::{Arc, Mutex, mpsc};

/// Start MPRIS against the daemon's `RemotePlayer` (#175, previously done in
/// `main.rs::run_remote_app` before the constructor ran). Moved here so App
/// owns the resulting handle and can `rebind` it later if
/// `switch_to_direct_remote` / `restore_local_mode` swap which target owns
/// playback.
///
/// Test builds leave `mpris` unset (`build()` initializes it to None):
/// `mpris::start` claims `org.mpris.MediaPlayer2.mbv` on the real D-Bus
/// session bus from a thread with no shutdown path -- leaked into every test
/// process that constructs a remote App, where process teardown races it
/// (issue #757). Tests leave `mpris` unset.
#[cfg(not(test))]
fn start_mpris(remote: &mbv_remote_player::RemotePlayer) -> mbv_desktop::mpris::MprisHandle {
    let mpris_remote = remote.clone();
    mbv_desktop::mpris::start(
        std::sync::Arc::clone(&mpris_remote.status),
        move |transport| mpris_remote.send_transport(transport),
        Some(remote.disconnected_flag()),
        crate::config::image_disk_cache_path,
    )
}

/// The daemon-side queue snapshot an attaching session seeds its queue
/// scope, local-daemon bootstrap, and queue tabs from.
struct RemoteSnapshot {
    unified_state: Option<mbv_ctrl::UnifiedQueueStateData>,
}

impl RemoteSnapshot {
    fn take(remote: &mbv_remote_player::RemotePlayer) -> Self {
        Self {
            unified_state: remote.unified_queue_state(),
        }
    }

    /// Whether the daemon side has any queue content to present.
    fn has_items(&self) -> bool {
        self.unified_state
            .as_ref()
            .is_some_and(|state| !state.slots.is_empty())
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
    fn local_bootstrap(&self) -> LocalDaemonBootstrap {
        self.unified_state
            .as_ref()
            .map_or_else(bootstrap_legacy_queue, bootstrap_unified_queue)
    }

    /// The queue tabs the session mounts: one unified tab for a local
    /// daemon, separate local/remote tabs for a network daemon.
    fn tabs(
        self,
        is_local: bool,
        local_daemon_bootstrap: Option<&LocalDaemonBootstrap>,
    ) -> (QueueView, Option<QueueView>) {
        if is_local {
            // Local daemon: one unified queue, exactly like plain local
            // playback — no separate remote_view, no scope pill.
            (
                local_daemon_bootstrap.as_ref().unwrap().local_view.clone(),
                None,
            )
        } else {
            // Remote/network daemon: keep a separate remote queue so the
            // user can browse locally while the daemon plays elsewhere.
            (
                QueueView::default(),
                Some(
                    self.unified_state
                        .map_or_else(QueueView::default, |state| QueueView::from_snapshot(&state)),
                ),
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
        mbv_config::load_service_secret(mbv_queue::ServiceKind::Emby).is_some();
    let audiobookshelf_configured = app_config.audiobookshelf_setup.is_some();
    let audiobookshelf_credential_present =
        mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf).is_some();
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

fn initialize_service_startup(
    app: &mut App,
    audiobookshelf_startup_requested: bool,
    emby_configured_without_client: bool,
    should_open_services: bool,
) {
    let config = app.config.lock().unwrap().clone();
    app.setup.emby_startup_request = emby_configured_without_client.then_some(
        crate::app::state::service_setup::StartupRequest {
            config: config.clone(),
            generation: app.emby_runtime.generation(),
        },
    );
    app.setup.audiobookshelf_startup_request = audiobookshelf_startup_requested.then_some(
        crate::app::state::service_setup::StartupRequest {
            config,
            generation: app.audiobookshelf_runtime.generation(),
        },
    );
    if should_open_services {
        app.open_services_settings();
    }
}

impl App {
    /// `endpoint` is the daemon endpoint the remote player is connected to.
    /// The endpoint's `is_local()` distinguishes local-daemon attach
    /// (`DaemonEndpoint::Local`) from a genuinely remote daemon:
    /// - `Local`: behaves like a plain local session — one unified queue,
    ///   normal queue-state persistence — the only difference is that the
    ///   daemon owns mpv instead of an in-process `Player`.
    /// - `Tcp`/`Unix`: a separate `remote_view` is kept so the user
    ///   can browse locally while a daemon elsewhere plays something else,
    ///   with the Local/Remote scope split (`[`/`]`) to switch between them.
    #[cfg(test)]
    pub fn new_remote_with_config(
        client: EmbyClient,
        remote: mbv_remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &DaemonEndpoint,
        config: crate::config::Config,
    ) -> Self {
        Self::new_remote_optional_with_config(Some(client), remote, player_rx, endpoint, config)
    }

    pub fn new_remote_optional_with_config(
        client: Option<EmbyClient>,
        remote: mbv_remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &DaemonEndpoint,
        app_config: crate::config::Config,
    ) -> Self {
        let (_, ws_rx) = mpsc::channel::<mbv_ws::WsEvent>();
        let (_, transport_rx) = mpsc::channel::<mbv_ctrl::TransportCommand>();
        let (card_image_tx, card_image_rx) =
            mpsc::channel::<(String, Option<image::DynamicImage>)>();
        let channels = crate::app::state::runtime_channels::RuntimeChannels::new();
        let ui_config = crate::config::load_ui_config().unwrap_or_default();
        let hidden_libraries = app_config.hidden_libraries.clone();
        let library_routes = app_config.library_routes.clone();
        let music_levels = app_config.music_levels.clone();
        let always_play_next = app_config.always_play_next;
        let emby_configured_without_client = client.is_none() && app_config.emby_setup.is_some();
        let should_open_services =
            crate::app::dispatch::session::service_startup::should_open_services(&app_config);
        let system_notifications = app_config.system_notifications;
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
        let snapshot = RemoteSnapshot::take(&remote);
        let initial_queue_scope = snapshot.scope(endpoint.is_local());
        let local_daemon_bootstrap = endpoint.is_local().then(|| snapshot.local_bootstrap());
        #[cfg(not(test))]
        let mpris_handle = Some(start_mpris(&remote));
        #[cfg(test)]
        let mpris_handle = None;
        let player = PlayerProxy::from_remote(remote, always_play_next);
        let audiobookshelf_startup_requested =
            services.audiobookshelf_configured && services.audiobookshelf_credential_present;
        let (local_view, remote_view) =
            snapshot.tabs(endpoint.is_local(), local_daemon_bootstrap.as_ref());
        let mut app = Self::build(AppInit {
            config,
            emby_runtime: services.emby_runtime,
            audiobookshelf_runtime: services.audiobookshelf_runtime,
            player,
            player_rx,
            ws_rx,
            transport_rx,
            ws_send_tx: None,
            audiobookshelf_socket_rx: detached_socket_rx(),
            audiobookshelf_socket_tx: None,
            audiobookshelf_socket_generation: None,
            local_view,
            remote_view,
            initial_queue_scope,
            system_notifications,
            image_protocol: ui_config.image_protocol.clone(),
            image_protocol_enabled: ui_config.image_protocol.is_some(),
            hidden_libraries,
            library_routes,
            music_levels,
            use_nerd_fonts: ui_config.use_nerd_fonts,
            indicator_style: ui_config.indicator_style.parse().unwrap_or_default(),
            image_cache_size: ui_config.image_cache_size,
            visualizer_glyph: ui_config.visualizer_glyph,
            card_image_tx,
            card_image_rx,
            channels,
            idle_feed: None,
        });
        app.mpris = mpris_handle;
        app.player_endpoint = Some(endpoint.clone());
        app.home_is_local_daemon = endpoint.is_local();
        app.sync_subtitle_prefs_to_player();
        app.launched_as_remote = true;
        debug_assert!(app.player_endpoint.is_some(), "player-endpoint invariant");
        if endpoint.is_local() {
            let bootstrap = local_daemon_bootstrap.unwrap();
            app.last_played_item_id = bootstrap.last_played_item_id;
            app.last_played_completed = bootstrap.last_played_completed;
            app.try_auto_reconnect();
        }
        initialize_service_startup(
            &mut app,
            audiobookshelf_startup_requested,
            emby_configured_without_client,
            should_open_services,
        );
        app
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn construct(config: crate::config::Config) -> App {
        let (remote, player_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);
        App::new_remote_optional_with_config(
            None,
            remote,
            player_rx,
            &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
            config,
        )
    }

    // toast-notification-semantics "Stay-alive does not suppress notifications":
    // the setting is followed through unchanged whatever stay-alive is.
    #[test]
    fn system_notifications_follow_config_when_stay_alive_is_on() {
        let _guard = crate::config::TestStateDirGuard::new();
        let config = crate::config::Config {
            stay_alive: true,
            system_notifications: true,
            ..Default::default()
        };
        let app = construct(config);

        assert!(app.system_notifications);
    }

    #[test]
    fn daemon_lifecycle_local_daemon_is_independent_of_emby_setup_requests_startup_when_configured_without_client()
     {
        let _guard = crate::config::TestStateDirGuard::new();
        let config = crate::config::Config {
            emby_setup: Some(mbv_config::EmbySetup::new("https://emby.example", "user")),
            ..Default::default()
        };
        let app = construct(config);

        assert!(app.setup.emby_startup_request.is_some());
    }
}
