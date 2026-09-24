use super::bootstrap::LocalDaemonBootstrap;
use super::types_playback::QueueScope;
use super::types_player_tab::PlayerTab;
use super::types_settings::{PanelFocus, PanelMode};
use super::types_tab_selection::TabSelection;
use super::{
    bootstrap_unified_queue, layout, render, spawn_resize_worker, App, AppInit, SessionEvent,
    SuspendedLocalSession, LEFT_WIDTH_DEFAULT,
};
use mbv_core::api::{EmbyClient, EmbyItem};
use mbv_core::player::{Player, PlayerEvent, PlayerProxy};
use mbv_core::remote_player::DaemonEndpoint;
use mbv_core::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use ratatui_image::picker::Picker;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

fn list_pane_width_from_prefs(prefs: &serde_json::Value) -> Option<u16> {
    prefs["list_pane_width"].as_u64().map(|v| v as u16)
}

impl App {
    /// Construct a local player and its worker channels through the ordinary
    /// startup path. The fall-through path uses this before it tears down an
    /// attached owner.
    pub(super) fn construct_local_session(&self) -> Result<SuspendedLocalSession, String> {
        #[cfg(test)]
        if let Some(prepare) = *super::LOCAL_PLAYER_PREPARE_OVERRIDE.lock().unwrap() {
            prepare()?;
        }
        let config = self.config.lock().unwrap().clone();
        let (player_tx, player_rx) = mpsc::channel();
        let raw_player = Player::new(
            String::new(),
            String::new(),
            config.show_audio_window,
            config.use_mpv_config,
            config.no_scripts,
            config.always_skip_intro,
            mbv_core::player::SubtitlePrefs {
                mode: config.subtitle_mode.clone(),
                subtitle_lang: config.subtitle_lang.clone(),
                audio_lang: config.audio_lang.clone(),
            },
            player_tx,
            None,
        )
        .with_video_cache(config.video_cache_forward_mb, config.video_cache_back_mb);
        let (_ws_tx, ws_rx) = mpsc::channel();
        let (_abs_tx, abs_rx) = mpsc::channel();
        let player = PlayerProxy::local(raw_player, config.always_play_next);
        // Test builds must never construct the real external (issue #757):
        // a fall-through test that plays locally would otherwise cold-start
        // a real mpv handle that races process teardown.
        #[cfg(test)]
        player.inhibit_mpv();
        Ok(SuspendedLocalSession {
            player,
            player_rx,
            ws_rx,
            ws_send_tx: None,
            audiobookshelf_socket_rx: abs_rx,
            audiobookshelf_socket_tx: None,
            audiobookshelf_socket_generation: None,
        })
    }

    pub(super) fn build(init: AppInit) -> Self {
        // Must run before `load_prefs()`: the guard redirects `config_dir()`/
        // `state_dir()` to an isolated tmpdir, and `load_prefs()` resolves
        // its path through that same lookup. Installing the guard after
        // reading prefs left tests reading (and initializing state from)
        // the real on-disk prefs.json instead of a fresh one.
        #[cfg(test)]
        let _test_state_dir_guard = crate::config::TestStateDirGuard::new_if_unset();
        let prefs = Self::load_prefs();
        let pending_launch_state = mbv_core::config::load_tui_launch_state();
        // The legacy selected-tab preference is only a migration input. Never
        // let it compete with a versioned launch snapshot that already exists.
        let legacy_launch_tab = pending_launch_state
            .is_none()
            .then(|| {
                prefs["library_tab"]
                    .as_u64()
                    .or_else(|| prefs["power_left_tab"].as_u64())
                    .and_then(|position| usize::try_from(position).ok())
            })
            .flatten();
        let bare_owner = mbv_core::player_owner_state::PlayerOwnerState::new(
            init.player_tab.queue.clone(),
            crate::config::QueueSource::Unknown,
        );
        let (resize_register_tx, resize_response_rx) = spawn_resize_worker();
        let (cast_tx, cast_rx) = mpsc::channel();
        let mut app = App {
            #[cfg(test)]
            _test_state_dir_guard,
            config: init.config,
            emby_runtime: init.emby_runtime,
            audiobookshelf_runtime: init.audiobookshelf_runtime,
            emby_startup_rx: init.emby_startup_rx,
            emby_startup_request: init.emby_startup_request,
            audiobookshelf_startup_rx: init.audiobookshelf_startup_rx,
            audiobookshelf_startup_request: init.audiobookshelf_startup_request,
            audiobookshelf_catalog_rx: None,
            audiobookshelf_libraries: Vec::new(),
            audiobookshelf_shelf_cache: std::collections::HashMap::new(),
            audiobookshelf_browse: Vec::new(),
            audiobookshelf_book_browse: Vec::new(),
            audiobookshelf_test_rx: init.audiobookshelf_test_rx,
            audiobookshelf_setup_rx: init.audiobookshelf_setup_rx,
            emby_setup_form: init.emby_setup_form,
            audiobookshelf_setup_form: None,
            emby_setup_rx: init.emby_setup_rx,
            pending_emby_replacement: None,
            pending_audiobookshelf_replacement: None,
            player: init.player,
            bare_owner,
            mpris: None,
            player_rx: init.player_rx,
            ws_rx: init.ws_rx,
            ws_send_tx: init.ws_send_tx,
            audiobookshelf_socket_rx: init.audiobookshelf_socket_rx,
            audiobookshelf_socket_tx: init.audiobookshelf_socket_tx,
            audiobookshelf_socket_generation: init.audiobookshelf_socket_generation,
            player_tab: init.player_tab,
            remote_player_tab: init.remote_player_tab,
            system_notifications: init.system_notifications,
            image_protocol: init.image_protocol,
            image_protocol_enabled: init.image_protocol_enabled,
            library_position_state: crate::config::load_library_position_state(),
            hidden_libraries: init.hidden_libraries,
            library_routes: init.library_routes,
            home_latest_launch_window: super::home_latest::HomeLatestLaunchWindow {
                previous: None,
                current: 0,
            },
            music_levels: init.music_levels,
            album_indexes: std::collections::HashMap::new(),
            use_nerd_fonts: init.use_nerd_fonts,
            indicator_style: init.indicator_style,
            image_cache_size: init.image_cache_size,
            lib_tx: init.lib_tx,
            lib_rx: init.lib_rx,
            search_tx: init.search_tx,
            search_rx: init.search_rx,
            sessions_tx: init.sessions_tx,
            sessions_rx: init.sessions_rx,
            card_image_tx: init.card_image_tx,
            card_image_rx: init.card_image_rx,
            resize_register_tx,
            resize_response_rx,
            notif_action_tx: init.notif_action_tx,
            notif_action_rx: init.notif_action_rx,
            libs: Vec::new(),
            status: String::new(),
            status_expires: None,
            status_severity: super::notify_actions::ToastSeverity::default(),
            layout: layout::AppLayout::default(),
            terminal_width: 80,
            terminal_height: 24,

            pending_overlay: None,
            pending_exit_message: None,
            pending_delete_slot: None,
            queue_undo_stack: Vec::new(),
            remote_queue_undo_stack: Vec::new(),
            pending_remote_move_cursor: None,
            pending_queue_edit_cursor: None,
            pending_queue_cursor_reanchor: None,
            next_up_item: None,
            // #361: read the new prefs key, falling back to the pre-#361 one
            // for one release. `power_focus`/`power_left_tab`/`power_left_width`
            // on disk are renamed to `panel_focus`/`library_tab`/`queue_column_width`;
            // this fallback can be deleted a release after that lands.
            panel_focus: PanelFocus::from_pref(
                prefs["panel_focus"]
                    .as_str()
                    .or_else(|| prefs["power_focus"].as_str()),
            ),
            tab: TabSelection::Home,
            queue_column_width: prefs["queue_column_width"]
                .as_u64()
                .or_else(|| prefs["power_left_width"].as_u64())
                .map(|v| (v as u16).max(LEFT_WIDTH_DEFAULT))
                .unwrap_or(LEFT_WIDTH_DEFAULT),
            list_pane_width: list_pane_width_from_prefs(&prefs),
            panel_mode: PanelMode::default(),
            // Mini view always starts on the queue panel; not persisted.
            mini_view_focus: PanelFocus::Queue,
            // Always start on Home until the live catalog resolves the
            // stable pending launch tab. The saved queue is restored
            // independently; destination state remains pending for task 3.2.
            library_tab_pending: 0,
            pending_launch_tab_resolved: false,
            pending_launch_state,
            legacy_launch_tab,
            legacy_launch_migration_attempted: false,
            emby_catalog_ready: false,
            audiobookshelf_catalog_ready: false,
            pending_navigate_tab_switch: None,
            pending_series_landing: None,
            pending_series_handoff: None,
            pending_track_selection: None,
            ui_volume: prefs["ui_volume"].as_u64().unwrap_or(100).min(200) as u8,
            pre_mute_volume: prefs["pre_mute_volume"].as_u64().map(|v| v as u8),
            mute_on: prefs["mute_on"].as_bool().unwrap_or(false),
            // Visualizer selection is session-local; every launch starts on
            // artwork so a stale visualizer choice never blanks the card.
            visualizer_enabled: false,
            visualizer_failed: false,
            visualizer: None,
            visualizer_window: Default::default(),
            visualizer_glyph: init.visualizer_glyph,
            marquee_text: String::new(),
            marquee_started_at: std::time::Instant::now(),
            last_played_item_id: None,
            last_played_completed: false,
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
            last_card_height: 0,
            last_card_width: 0,
            queue_card_projection:
                crate::app::render::components::card::QueueCardProjection::default(),
            image_picker: None,
            halfblock_picker: None,
            dim_backdrop_active: false,
            image_cache_size_total: init.image_cache_size.saturating_mul(2),
            settings_destination: super::types_settings::SettingsDestination::Main,
            settings_save_at: None,
            mouse_capture_pending: None,
            confirm_logout: false,
            notif_failed: false,
            sessions: Vec::new(),
            cast_receivers: Vec::new(),
            panel_targets: Vec::new(),
            sessions_loading: false,
            playlists: Vec::new(),
            playlists_cursor: 0,
            playlists_scroll: 0,
            playlists_loading: false,
            playlists_open: None,
            playlists_open_items: Vec::new(),
            playlists_open_cursor: 0,
            playlists_open_scroll: 0,
            playlists_open_loading: false,
            queue_source: crate::config::QueueSource::Unknown,
            queue_dirty: false,
            pending_owner_source_update: None,
            pending_queue_action: None,
            pending_queue_replacement: None,
            pending_local_play: None,
            last_keepalive: Instant::now(),
            last_capabilities: Instant::now(),
            connected_session_id: None,
            connected_session_state: None,
            cast_attachment: None,
            cast_tx,
            cast_rx,
            last_cast_poll: Instant::now() - Duration::from_secs(60),
            cast_status_loading: false,
            remote_queue_lineage: 0,
            playlist_mutations: std::collections::HashMap::new(),
            next_playlist_mutation: 1,
            next_owner_queue_load_request: 1,
            direct_remote_connected: false,
            direct_remote_label: None,
            direct_remote_session_id: None,
            last_session_poll: Instant::now() - Duration::from_secs(60),
            session_miss_count: 0,
            remote_pos_s: 0,
            remote_pos_at: Instant::now(),
            remote_api_pos_advanced_at: Instant::now() - Duration::from_secs(60),
            remote_stalled_while_paused: false,
            remote_seek_pending_until: Instant::now() - Duration::from_secs(1),
            runtime_zero_since: None,
            suspended_local: None,
            active_route: None,
            library_route_cache: std::collections::HashMap::new(),
            force_clear: false,
            prefix_armed: false,
            tab_scroll: 0,
            last_nav_at: Instant::now() - Duration::from_secs(1),
            last_library_nav_at: Instant::now() - Duration::from_secs(1),
            refocus_at: None,
            album_artist_cache: std::collections::HashMap::new(),
            album_artist_levels: std::collections::HashMap::new(),
            pending_level_artist_warmups: std::collections::VecDeque::new(),
            level_artist_warmups_in_flight: std::collections::HashSet::new(),
            album_tracks_cache: std::collections::HashMap::new(),
            album_tracks_loading: std::collections::HashSet::new(),
            pending_artist_album_track_fetches: std::collections::VecDeque::new(),
            artist_album_track_fetches_in_flight: std::collections::HashSet::new(),
            artist_detail_cache: std::collections::HashMap::new(),
            artist_detail_loading: std::collections::HashSet::new(),
            artist_artwork_requests: std::collections::HashMap::new(),
            artist_artwork_status: std::collections::HashMap::new(),
            series_detail_cache: std::collections::HashMap::new(),
            series_detail_loading: std::collections::HashSet::new(),
            series_season_loading: std::collections::HashSet::new(),
            pending_series_season_expansions: std::collections::HashSet::new(),
            image_lru: std::collections::VecDeque::new(),
            pending_image_fetches: std::collections::VecDeque::new(),
            image_fetches_active: 0,
            queue_scope: init.initial_queue_scope,
            launched_as_remote: false,
            player_endpoint: None,
            home_is_local_daemon: false,
            idle_feed: init.idle_feed,
            feed_seek_pending_slot: None,
            feed_tab: super::types_feed_tab::FeedTabState::default(),
            feed_entry_state: mbv_core::feed_entry_state::FeedEntryStore::load(),
            #[cfg(test)]
            card_image_fetch_calls: 0,
            #[cfg(test)]
            image_protocol_builds: std::cell::Cell::new(0),
        };
        app.sync_feed_subscriptions();
        app
    }

    /// Construct the bare Player owner without creating an Emby client or
    /// performing any network work. Configured Emby setup is initialized by
    /// the bounded worker once `run()` has entered the TUI.
    pub fn new_independent(app_config: crate::config::Config) -> Self {
        let (player_tx, player_rx) = mpsc::channel();
        let (_, ws_rx) = mpsc::channel();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel::<SessionEvent>();
        let (card_image_tx, card_image_rx) =
            mpsc::channel::<(String, Option<image::DynamicImage>)>();
        let (notif_action_tx, notif_action_rx) = mpsc::channel::<String>();
        let (search_tx, search_rx) = mpsc::channel::<(String, Result<Vec<EmbyItem>, String>)>();
        let ui_config = crate::config::load_ui_config().unwrap_or_default();
        let indicator_style = ui_config.indicator_style.parse().unwrap_or_default();
        let configured = app_config.emby_setup.is_some();
        let credential_present =
            mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby).is_some();
        let generation = mbv_core::service_runtime::SetupGeneration::default();
        let audiobookshelf_configured = app_config.audiobookshelf_setup.is_some();
        let audiobookshelf_credential_present =
            mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
                .is_some();
        let raw_player = Player::new(
            String::new(),
            String::new(),
            app_config.show_audio_window,
            app_config.use_mpv_config,
            app_config.no_scripts,
            app_config.always_skip_intro,
            mbv_core::player::SubtitlePrefs {
                mode: app_config.subtitle_mode.clone(),
                subtitle_lang: app_config.subtitle_lang.clone(),
                audio_lang: app_config.audio_lang.clone(),
            },
            player_tx,
            None,
        );
        let raw_player = raw_player.with_video_cache(
            app_config.video_cache_forward_mb,
            app_config.video_cache_back_mb,
        );
        let player = PlayerProxy::local(raw_player, app_config.always_play_next);
        let mut app = Self::build(AppInit {
            config: Arc::new(Mutex::new(app_config.clone())),
            emby_runtime: {
                let mut runtime = EmbyRuntime::new(configured);
                runtime.state =
                    super::service_startup::initial_state(configured, credential_present);
                runtime
            },
            audiobookshelf_runtime: {
                let mut runtime = AudiobookshelfRuntime::new(audiobookshelf_configured);
                runtime.state = super::service_startup::audiobookshelf_initial_state(
                    audiobookshelf_configured,
                    audiobookshelf_credential_present,
                );
                runtime
            },
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
            audiobookshelf_socket_rx: {
                let (_, rx) = mpsc::channel();
                rx
            },
            audiobookshelf_socket_tx: None,
            audiobookshelf_socket_generation: None,
            player_tab: PlayerTab::default(),
            remote_player_tab: None,
            initial_queue_scope: QueueScope::Local,
            system_notifications: app_config.system_notifications,
            image_protocol: ui_config.image_protocol.clone(),
            image_protocol_enabled: ui_config.image_protocol.is_some(),
            hidden_libraries: app_config.hidden_libraries.clone(),
            library_routes: app_config.library_routes.clone(),
            music_levels: app_config.music_levels.clone(),
            use_nerd_fonts: ui_config.use_nerd_fonts,
            indicator_style,
            image_cache_size: ui_config.image_cache_size,
            visualizer_glyph: ui_config.visualizer_glyph.clone(),
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
        app.emby_startup_request = configured.then_some((app_config.clone(), generation));
        app.audiobookshelf_startup_request = (audiobookshelf_configured
            && audiobookshelf_credential_present)
            .then_some((app_config.clone(), generation));
        if super::service_startup::should_open_services(&app_config) {
            app.open_services_settings();
        }
        app
    }

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
        endpoint: DaemonEndpoint,
    ) -> Self {
        let config = crate::config::load_config().unwrap_or_default();
        Self::new_remote_optional_with_config(Some(client), remote, player_rx, endpoint, config)
    }

    #[cfg(test)]
    pub fn new_remote_with_config(
        client: EmbyClient,
        remote: mbv_core::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: DaemonEndpoint,
        config: crate::config::Config,
    ) -> Self {
        Self::new_remote_optional_with_config(Some(client), remote, player_rx, endpoint, config)
    }

    pub fn new_remote_optional_with_config(
        client: Option<EmbyClient>,
        remote: mbv_core::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: DaemonEndpoint,
        app_config: crate::config::Config,
    ) -> Self {
        let (_, ws_rx) = mpsc::channel::<mbv_core::ws::WsEvent>();
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
        let image_protocol = ui_config.image_protocol.clone();
        let image_protocol_enabled = image_protocol.is_some();
        let image_cache_size = ui_config.image_cache_size;
        let use_nerd_fonts = ui_config.use_nerd_fonts;
        let indicator_style: render::indicators::IndicatorStyle =
            ui_config.indicator_style.parse().unwrap_or_default();
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
        let emby_configured = app_config.emby_setup.is_some();
        let emby_credential_present =
            mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby).is_some();
        let audiobookshelf_configured = app_config.audiobookshelf_setup.is_some();
        let audiobookshelf_credential_present =
            mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
                .is_some();
        let config = Arc::new(Mutex::new(app_config));
        let client_arc = client.map(|client| Arc::new(Mutex::new(client)));
        let remote_items = remote.items.lock().unwrap().clone();
        let remote_cursor = remote.status.lock().unwrap().current_idx;
        let remote_unified_state = remote.unified_queue_state();
        let remote_queue_source = remote.queue_source.lock().unwrap().clone();
        let remote_has_items = remote_unified_state
            .as_ref()
            .map_or(!remote_items.is_empty(), |state| !state.slots.is_empty());
        let initial_queue_scope = if !endpoint.is_local() && remote_has_items {
            QueueScope::Remote
        } else {
            QueueScope::Local
        };
        let local_daemon_bootstrap = endpoint.is_local().then(|| {
            remote_unified_state.as_ref().map_or_else(
                || LocalDaemonBootstrap {
                    player_tab: PlayerTab::from_emby_items(remote_items.clone(), remote_cursor),
                    queue_source: remote_queue_source.clone(),
                    last_played_item_id: None,
                    last_played_completed: false,
                },
                bootstrap_unified_queue,
            )
        });
        // Start MPRIS against this `RemotePlayer` (#175, previously done in
        // `main.rs::run_remote_app` before this constructor even ran).
        // Moved here so App owns the resulting handle and can `rebind` it
        // later if `switch_to_direct_remote` / `restore_local_mode` swap
        // which target owns playback.
        // Test builds leave `mpris` unset (build() initializes it to None):
        // `mpris::start` claims `org.mpris.MediaPlayer2.mbv` on the real
        // D-Bus session bus from a thread with no shutdown path -- leaked
        // into every test process that constructs a remote App, where
        // process teardown races it (issue #757). Tests that exercise
        // rebind inject `mpris::test_handle` themselves.
        #[cfg(not(test))]
        let mpris_handle = {
            let mpris_remote = remote.clone();
            Some(crate::mpris::start(
                mpris_remote.status.clone(),
                move |cmd| {
                    mpris_remote.send_command(cmd);
                },
                Some(remote.disconnected_flag()),
            ))
        };
        #[cfg(test)]
        let mpris_handle = None;
        let player = PlayerProxy::remote(remote, always_play_next);
        let (player_tab, remote_player_tab) = if endpoint.is_local() {
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
                Some(remote_unified_state.as_ref().map_or_else(
                    || PlayerTab::from_emby_items(remote_items, remote_cursor),
                    PlayerTab::from_unified_state,
                )),
            )
        };
        let mut app = Self::build(AppInit {
            config,
            emby_runtime: client_arc.as_ref().map_or_else(
                || {
                    let mut runtime = EmbyRuntime::new(emby_configured);
                    runtime.state = super::service_startup::initial_state(
                        emby_configured,
                        emby_credential_present,
                    );
                    runtime
                },
                |client| EmbyRuntime::ready(client.clone()),
            ),
            audiobookshelf_runtime: {
                let mut runtime = AudiobookshelfRuntime::new(audiobookshelf_configured);
                runtime.state = super::service_startup::audiobookshelf_initial_state(
                    audiobookshelf_configured,
                    audiobookshelf_credential_present,
                );
                runtime
            },
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
            audiobookshelf_socket_rx: {
                let (_, rx) = mpsc::channel();
                rx
            },
            audiobookshelf_socket_tx: None,
            audiobookshelf_socket_generation: None,
            player_tab,
            remote_player_tab,
            initial_queue_scope,
            system_notifications: false,
            image_protocol,
            image_protocol_enabled,
            hidden_libraries,
            library_routes,
            music_levels,
            use_nerd_fonts,
            indicator_style,
            image_cache_size,
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
        } else {
            app.queue_source = remote_queue_source;
        }
        if endpoint.is_local() {
            app.try_auto_reconnect();
        }
        let generation = app.audiobookshelf_runtime.generation();
        app.audiobookshelf_startup_request = (audiobookshelf_configured
            && audiobookshelf_credential_present)
            .then_some((app.config.lock().unwrap().clone(), generation));
        app
    }

    /// Query the terminal for its image protocol (sixel/kitty/iterm2/etc,
    /// via `Picker::from_query_stdio`, falling back to halfblocks), then
    /// apply `self.image_protocol`'s override if it names one of the known
    /// protocols. Called once at startup by `run`.
    pub(super) fn build_image_picker(&self) -> Picker {
        use ratatui_image::picker::ProtocolType;
        let protocol_override = self.image_protocol.clone();
        let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let proto = protocol_override
            .as_deref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "sixel" => Some(ProtocolType::Sixel),
                "kitty" => Some(ProtocolType::Kitty),
                "iterm2" => Some(ProtocolType::Iterm2),
                "halfblocks" => Some(ProtocolType::Halfblocks),
                _ => None, // "auto" or unknown: use picker's detected protocol
            });
        if let Some(proto) = proto {
            picker.set_protocol_type(proto);
        }
        picker
    }

    /// Populate `image_picker` (terminal-detected, with the config override)
    /// and `halfblock_picker` (the #451 dimmed-backdrop fallback: modals
    /// re-encode images to halfblocks so the dim applies uniformly).
    ///
    /// MUST run before the TuiRealm crossterm listener starts
    /// (`Application::init`): `Picker::from_query_stdio` writes a
    /// `CSI 16 t` cell-size query to the terminal and reads the reply with a
    /// raw `io::stdin().read()`. If the listener thread is already draining
    /// stdin it eats the reply, the picker falls back to a wrong cell size,
    /// and Kitty renders images clipped on the right/bottom (#654).
    pub(crate) fn init_image_pickers(&mut self) {
        let picker = self.build_image_picker();
        log::debug!(
            target: "startup",
            "image picker: protocol={:?} font_size={:?}",
            picker.protocol_type(),
            picker.font_size()
        );
        self.image_picker = Some(picker);
        self.halfblock_picker = Some(Picker::halfblocks());
    }
}

#[cfg(test)]
mod tests {
    use super::list_pane_width_from_prefs;

    #[test]
    fn list_pane_width_pref_accepts_width_and_rejects_empty_or_invalid_values() {
        assert_eq!(
            list_pane_width_from_prefs(&serde_json::json!({ "list_pane_width": 42 })),
            Some(42)
        );
        assert_eq!(
            list_pane_width_from_prefs(&serde_json::json!({ "list_pane_width": null })),
            None
        );
        assert_eq!(list_pane_width_from_prefs(&serde_json::json!({})), None);
        assert_eq!(
            list_pane_width_from_prefs(&serde_json::json!({ "list_pane_width": "42" })),
            None
        );
    }
}
