use crate::app::state::types::playback::QueueScope;
use crate::app::state::types::player_tab::PlayerTab;
use crate::app::state::types::settings::{PanelFocus, PanelMode};
use crate::app::state::types::tab_selection::TabSelection;
use crate::app::{
    layout, spawn_resize_worker, App, AppInit, SuspendedLocalSession, LEFT_WIDTH_DEFAULT,
};
use mbv_core::player::{Player, PlayerProxy};
use mbv_core::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

mod remote;

fn list_pane_width_from_prefs(prefs: &serde_json::Value) -> Option<u16> {
    prefs["list_pane_width"]
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
}

fn visual_slot_hidden_from_prefs(prefs: &serde_json::Value) -> bool {
    prefs["visual_slot_hidden"].as_bool().unwrap_or(false)
}

/// The idle Emby service runtime an independent (non-daemon) session starts
/// from: configured with or without a stored credential.
fn independent_emby_runtime(configured: bool, credential_present: bool) -> EmbyRuntime {
    let mut runtime = EmbyRuntime::new(configured);
    runtime.state = crate::app::dispatch::session::service_startup::initial_state(
        configured,
        credential_present,
    );
    runtime
}

/// The idle Audiobookshelf service runtime an independent or attaching
/// session starts from: configured with or without a stored credential.
fn independent_audiobookshelf_runtime(
    configured: bool,
    credential_present: bool,
) -> AudiobookshelfRuntime {
    let mut runtime = AudiobookshelfRuntime::new(configured);
    runtime.state = crate::app::dispatch::session::service_startup::audiobookshelf_initial_state(
        configured,
        credential_present,
    );
    runtime
}

/// A detached Audiobookshelf socket receiver: sessions that do not host the
/// ABS socket loop still need the channel `App` carries.
fn detached_socket_rx() -> mpsc::Receiver<mbv_core::audiobookshelf::socket::SocketEvent> {
    let (_, rx) = mpsc::channel();
    rx
}

impl App {
    /// Construct a local player and its worker channels through the ordinary
    /// startup path. The fall-through path uses this before it tears down an
    /// attached owner.
    pub(in crate::app) fn construct_local_session(&self) -> SuspendedLocalSession {
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
        SuspendedLocalSession {
            player,
            player_rx,
            ws_rx,
            ws_send_tx: None,
            audiobookshelf_socket_rx: abs_rx,
            audiobookshelf_socket_tx: None,
            audiobookshelf_socket_generation: None,
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "App construction explicitly initializes heterogeneous fields after extracting four owned seams"
    )]
    pub(in crate::app) fn build(init: AppInit) -> Self {
        // Must run before `load_prefs()`: the guard redirects `config_dir()`/
        // `state_dir()` to an isolated tmpdir, and `load_prefs()` resolves
        // its path through that same lookup. Installing the guard after
        // reading prefs left tests reading (and initializing state from)
        // the real on-disk prefs.json instead of a fresh one.
        #[cfg(test)]
        let test_state_dir_guard = crate::config::TestStateDirGuard::new_if_unset();
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
        let setup = crate::app::state::service_setup::ServiceSetup::new();
        let mut app = App {
            #[cfg(test)]
            _test_state_dir_guard: test_state_dir_guard,
            config: init.config,
            emby_runtime: init.emby_runtime,
            audiobookshelf_runtime: init.audiobookshelf_runtime,
            setup,
            channels: init.channels,
            audiobookshelf_libraries: Vec::new(),
            audiobookshelf_shelf_cache: std::collections::HashMap::new(),
            audiobookshelf_browse: Vec::new(),
            audiobookshelf_book_browse: Vec::new(),
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
            images: crate::app::infra::images::cache::ImageCache::new(
                init.image_cache_size,
                init.image_protocol,
                init.image_protocol_enabled,
                init.card_image_tx,
                init.card_image_rx,
                resize_register_tx,
                resize_response_rx,
            ),
            library_position_state: crate::config::load_library_position_state(),
            hidden_libraries: init.hidden_libraries,
            library_routes: init.library_routes,
            home_latest_launch_window: crate::app::state::home_latest::HomeLatestLaunchWindow {
                previous: None,
                current: 0,
            },
            music_levels: init.music_levels,
            album_indexes: std::collections::HashMap::new(),
            use_nerd_fonts: init.use_nerd_fonts,
            indicator_style: init.indicator_style,
            libs: Vec::new(),
            status: String::new(),
            status_expires: None,
            status_severity: crate::app::dispatch::notify::ToastSeverity::default(),
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
                .map_or(LEFT_WIDTH_DEFAULT, |value| {
                    u16::try_from(value)
                        .unwrap_or(u16::MAX)
                        .max(LEFT_WIDTH_DEFAULT)
                }),
            list_pane_width: list_pane_width_from_prefs(&prefs),
            visual_slot_hidden: visual_slot_hidden_from_prefs(&prefs),
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
            pre_mute_volume: prefs["pre_mute_volume"]
                .as_u64()
                .map(|value| u8::try_from(value).unwrap_or(u8::MAX)),
            mute_on: prefs["mute_on"].as_bool().unwrap_or(false),
            // Visualizer selection is session-local; every launch starts on
            // artwork so a stale visualizer choice never blanks the card.
            visualizer_enabled: false,
            visualizer_failed: false,
            visualizer: None,
            visualizer_window: mbv_visualizer::StereoSampleWindow::default(),
            visualizer_glyph: init.visualizer_glyph,
            last_played_item_id: None,
            last_played_completed: false,
            queue_card_projection:
                crate::app::render::components::card::QueueCardProjection::default(),
            dim_backdrop_active: false,
            settings_destination: crate::app::state::types::settings::SettingsDestination::Main,
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
            last_cast_poll: Instant::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or_else(Instant::now),
            cast_status_loading: false,
            queue_epoch: crate::app::state::queue_owner::QueueEpoch::default(),
            playlist_mutations: std::collections::HashMap::new(),
            next_playlist_mutation: 1,
            next_owner_queue_load_request: 1,
            remote: crate::app::state::remote_tracking::RemoteTracking::new(),
            suspended_local: None,
            active_route: None,
            library_route_cache: std::collections::HashMap::new(),
            force_clear: false,
            prefix_armed: false,
            tab_scroll: 0,
            last_nav_at: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
            last_library_nav_at: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
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
            queue_scope: init.initial_queue_scope,
            launched_as_remote: false,
            player_endpoint: None,
            home_is_local_daemon: false,
            idle_feed: init.idle_feed,
            feed_seek_pending_slot: None,
            feed_tab: crate::app::state::types::feed_tab::FeedTabState::default(),
            feed_entry_state: mbv_core::feed_entry_state::FeedEntryStore::load(),
        };
        app.sync_feed_subscriptions();
        app
    }

    /// Construct the bare Player owner without creating an Emby client or
    /// performing any network work. Configured Emby setup is initialized by
    /// the bounded worker once `run()` has entered the TUI.
    pub fn new_independent(app_config: &crate::config::Config) -> Self {
        let (player_tx, player_rx) = mpsc::channel();
        let (_, ws_rx) = mpsc::channel();
        let (card_image_tx, card_image_rx) =
            mpsc::channel::<(String, Option<image::DynamicImage>)>();
        let channels = crate::app::state::runtime_channels::RuntimeChannels::new();
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
            emby_runtime: independent_emby_runtime(configured, credential_present),
            audiobookshelf_runtime: independent_audiobookshelf_runtime(
                audiobookshelf_configured,
                audiobookshelf_credential_present,
            ),
            player,
            player_rx,
            ws_rx,
            ws_send_tx: None,
            audiobookshelf_socket_rx: detached_socket_rx(),
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
            card_image_tx,
            card_image_rx,
            channels,
            idle_feed: None,
        });
        app.setup.emby_startup_request = configured.then_some((app_config.clone(), generation));
        app.setup.audiobookshelf_startup_request = (audiobookshelf_configured
            && audiobookshelf_credential_present)
            .then_some((app_config.clone(), generation));
        if crate::app::dispatch::session::service_startup::should_open_services(app_config) {
            app.open_services_settings();
        }
        app
    }

    /// Initialize terminal image pickers before the TUI listener starts.
    pub(crate) fn init_image_pickers(&mut self) {
        self.images.init_image_pickers();
    }
}
