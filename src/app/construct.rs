use super::types_playback::{HomePane, QueueScope};
use super::types_player_tab::PlayerTab;
use super::types_settings::{PanelFocus, PanelMode};
use super::types_tab_selection::TabSelection;
use super::{
    bootstrap_local_daemon_queue, layout, render, spawn_resize_worker, App, AppInit, SessionEvent,
    LEFT_WIDTH_DEFAULT,
};
use mbv_core::api::{EmbyClient, EmbyItem};
use mbv_core::player::{Player, PlayerEvent, PlayerProxy};
use mbv_core::remote_player::DaemonEndpoint;
use ratatui_image::picker::Picker;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

impl App {
    pub(super) fn build(init: AppInit) -> Self {
        // Must run before `load_prefs()`: the guard redirects `config_dir()`/
        // `state_dir()` to an isolated tmpdir, and `load_prefs()` resolves
        // its path through that same lookup. Installing the guard after
        // reading prefs left tests reading (and initializing state from)
        // the real on-disk prefs.json instead of a fresh one.
        #[cfg(test)]
        let _test_state_dir_guard = crate::config::TestStateDirGuard::new_if_unset();
        let prefs = Self::load_prefs();
        let (resize_register_tx, resize_response_rx) = spawn_resize_worker();
        App {
            #[cfg(test)]
            _test_state_dir_guard,
            client: init.client,
            shared_client: None,
            shared_reconnect_rx: None,
            player: init.player,
            mpris: None,
            player_rx: init.player_rx,
            ws_rx: init.ws_rx,
            ws_send_tx: init.ws_send_tx,
            player_tab: init.player_tab,
            remote_player_tab: init.remote_player_tab,
            system_notifications: init.system_notifications,
            image_protocol: init.image_protocol,
            image_protocol_enabled: init.image_protocol_enabled,
            library_position_state: crate::config::load_library_position_state(),
            hidden_libraries: init.hidden_libraries,
            library_routes: init.library_routes,
            hidden_latest: init.hidden_latest,
            music_levels: init.music_levels,
            album_indexes: std::collections::HashMap::new(),
            use_nerd_fonts: init.use_nerd_fonts,
            indicator_style: init.indicator_style,
            image_cache_size: init.image_cache_size,
            lib_tx: init.lib_tx,
            lib_rx: init.lib_rx,
            search_tx: init.search_tx,
            search_rx: init.search_rx,
            search_debounce_deadline: None,
            search_debounce_pending: None,
            search_sidebar: None,
            sessions_tx: init.sessions_tx,
            sessions_rx: init.sessions_rx,
            card_image_tx: init.card_image_tx,
            card_image_rx: init.card_image_rx,
            resize_register_tx,
            resize_response_rx,
            notif_action_tx: init.notif_action_tx,
            notif_action_rx: init.notif_action_rx,
            home: HomePane {
                continue_items: Vec::new(),
                continue_cursor: 0,
                latest: Vec::new(),
                section: 0,
                home_cursor: 0,
                home_scroll: 0,
            },
            libs: Vec::new(),
            status: String::new(),
            status_expires: None,
            status_severity: super::notify_actions::ToastSeverity::default(),
            layout: layout::AppLayout::default(),
            terminal_width: 80,
            terminal_height: 24,

            home_loading: true,
            mouse_col: 0,
            mouse_row: 0,
            last_click_time: Instant::now(),
            last_drag_seek: Instant::now() - Duration::from_secs(1),
            last_space_press: None,
            last_esc_press: None,
            last_click_pos: (u16::MAX, u16::MAX),
            confirm_modal: None,
            daemon_lost_modal: None,
            pending_exit_message: None,
            pending_delete_slot: None,
            pending_queue_removal: None,
            queue_undo_stack: Vec::new(),
            remote_queue_undo_stack: Vec::new(),
            pending_remote_move_cursor: None,
            pending_queue_edit_cursor: None,
            pending_active_idx: None,
            skip_intro_end_ticks: None,
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
            panel_mode: PanelMode::default(),
            // Always start on Home. The saved queue is restored independently;
            // the saved library tab remains available for runtime persistence.
            library_tab_pending: 0,
            queue_scroll: 0,
            ui_volume: prefs["ui_volume"].as_u64().unwrap_or(100).min(200) as u8,
            pre_mute_volume: prefs["pre_mute_volume"].as_u64().map(|v| v as u8),
            mute_on: prefs["mute_on"].as_bool().unwrap_or(false),
            visualizer_enabled: prefs["visualizer_enabled"].as_bool().unwrap_or(false),
            visualizer_failed: false,
            visualizer: None,
            visualizer_frame: Vec::new(),
            now_playing_throbber: throbber_widgets_tui::ThrobberState::default(),
            last_throbber_advance: std::time::Instant::now(),
            last_played_item_id: None,
            last_played_completed: false,
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
            last_card_height: 0,
            last_card_width: 0,
            image_picker: None,
            halfblock_picker: None,
            dim_backdrop_active: false,
            image_cache_size_total: init.image_cache_size.saturating_mul(2),
            show_help: false,
            show_settings: false,
            settings_cursor: 0,
            settings_scroll: 0,
            settings_save_at: None,
            confirm_logout: false,
            multiselect_popup: None,
            library_routes_popup: None,
            help_scroll: 0,
            notif_failed: false,
            context_menu: None,
            sessions: Vec::new(),
            sessions_cursor: 0,
            sessions_scroll: 0,
            sessions_loading: false,
            show_sessions: false,
            playlists: Vec::new(),
            playlists_cursor: 0,
            playlists_scroll: 0,
            playlists_loading: false,
            show_playlists: false,
            playlists_open: None,
            playlists_open_items: Vec::new(),
            playlists_open_cursor: 0,
            playlists_open_scroll: 0,
            playlists_open_loading: false,
            queue_source: crate::config::QueueSource::Unknown,
            queue_dirty: false,
            pending_queue_action: None,
            remote_reanchor_popup: None,
            last_keepalive: Instant::now(),
            last_capabilities: Instant::now(),
            connected_session_id: None,
            connected_session_state: None,
            remote_tracker: None,
            remote_queue_projection: None,
            remote_queue_lineage: 0,
            playlist_mutations: std::collections::HashMap::new(),
            next_playlist_mutation: 1,
            session_poll_generation: 0,
            direct_remote_connected: false,
            direct_remote_label: None,
            last_session_poll: Instant::now() - Duration::from_secs(60),
            session_miss_count: 0,
            remote_pos_s: 0,
            remote_pos_at: Instant::now(),
            remote_api_pos_advanced_at: Instant::now() - Duration::from_secs(60),
            remote_seek_pending_until: Instant::now() - Duration::from_secs(1),
            runtime_zero_since: None,
            suspended_local: None,
            active_route: None,
            library_route_cache: std::collections::HashMap::new(),
            force_clear: false,
            tab_scroll: 0,
            last_scroll_at: Instant::now() - Duration::from_secs(1),
            last_nav_at: Instant::now() - Duration::from_secs(1),
            last_library_nav_at: Instant::now() - Duration::from_secs(1),
            library_position_dirty: false,
            library_position_dirty_at: Instant::now() - Duration::from_secs(1),
            refocus_at: None,
            album_artist_cache: std::collections::HashMap::new(),
            album_artist_loading: std::collections::HashSet::new(),
            pending_album_artist_fetches: std::collections::VecDeque::new(),
            album_artist_fetches_active: 0,
            album_tracks_cache: std::collections::HashMap::new(),
            album_tracks_loading: std::collections::HashSet::new(),
            series_detail_cache: std::collections::HashMap::new(),
            series_detail_loading: std::collections::HashSet::new(),
            save_playlist_dialog: None,
            image_lru: std::collections::VecDeque::new(),
            pending_image_fetches: std::collections::VecDeque::new(),
            image_fetches_active: 0,
            queue_scope: init.initial_queue_scope,
            launched_as_remote: false,
            player_endpoint: None,
            home_is_local_daemon: false,
            idle_feed: init.idle_feed,
        }
    }

    pub fn new(client: EmbyClient) -> Self {
        let (player_tx, player_rx) = mpsc::channel();
        let (ws_tx, ws_rx) = mpsc::channel();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel::<SessionEvent>();
        let (card_image_tx, card_image_rx) =
            mpsc::channel::<(String, Option<image::DynamicImage>)>();
        let (notif_action_tx, notif_action_rx) = mpsc::channel::<String>();
        let (search_tx, search_rx) = mpsc::channel::<(String, Result<Vec<EmbyItem>, String>)>();
        let ui_config = crate::config::load_ui_config().unwrap_or_default();
        let server_url = client.config.server_url.clone();
        let token = client.token.clone();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let library_routes = client.config.library_routes.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let system_notifications = client.config.system_notifications;
        let image_protocol = ui_config.image_protocol.clone();
        let image_protocol_enabled = image_protocol.is_some();
        let image_cache_size = ui_config.image_cache_size;
        let use_nerd_fonts = ui_config.use_nerd_fonts;
        let indicator_style: render::indicators::IndicatorStyle =
            ui_config.indicator_style.parse().unwrap_or_default();
        let always_play_next = client.config.always_play_next;
        let always_skip_intro = client.config.always_skip_intro;
        crate::config::evict_old_image_cache();
        let ws_url = client.ws_url();
        let ws_send_tx = mbv_core::ws::start(ws_url, ws_tx);
        let ws_send_tx_app = ws_send_tx.clone();
        // Prefer local config; fall back to Emby server prefs only on first run (all empty).
        let subtitle_prefs = if client.config.subtitle_mode.is_empty()
            && client.config.subtitle_lang.is_empty()
            && client.config.audio_lang.is_empty()
        {
            client.get_user_subtitle_prefs().unwrap_or_default()
        } else {
            mbv_core::player::SubtitlePrefs {
                mode: client.config.subtitle_mode.clone(),
                subtitle_lang: client.config.subtitle_lang.clone(),
                audio_lang: client.config.audio_lang.clone(),
            }
        };
        let raw_player = Player::new(
            server_url,
            token,
            client.config.show_audio_window,
            client.config.use_mpv_config,
            client.config.no_scripts,
            always_play_next,
            always_skip_intro,
            subtitle_prefs,
            player_tx,
            Some(ws_send_tx),
        );
        let player_status = raw_player.status.clone();
        let player_cmd_tx = raw_player.cmd_tx.clone();
        let mpris_handle = crate::mpris::start(
            player_status,
            move |cmd| {
                if let Some(tx) = player_cmd_tx.lock().unwrap().as_ref() {
                    let _ = tx.send(cmd);
                }
            },
            None,
        );
        let player = PlayerProxy::local(raw_player, always_play_next);
        let client_arc = Arc::new(Mutex::new(client));
        {
            let c = client_arc.clone();
            std::thread::spawn(move || {
                let mut probe = c.lock().unwrap().clone();
                probe.probe_chapter_api();
                c.lock().unwrap().chapter_api_available = probe.chapter_api_available;
            });
        }
        let mut app = Self::build(AppInit {
            client: client_arc,
            player,
            player_rx,
            ws_rx,
            ws_send_tx: Some(ws_send_tx_app),
            player_tab: PlayerTab::default(),
            remote_player_tab: None,
            initial_queue_scope: QueueScope::Local,
            system_notifications,
            image_protocol,
            image_protocol_enabled,
            hidden_libraries,
            library_routes,
            hidden_latest,
            music_levels,
            use_nerd_fonts,
            indicator_style,
            image_cache_size,
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
        app.mpris = Some(mpris_handle);
        app.initialize_shared_state();
        app.try_auto_reconnect();
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
    pub fn new_remote(
        client: EmbyClient,
        remote: mbv_core::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: DaemonEndpoint,
    ) -> Self {
        let (_, ws_rx) = mpsc::channel::<mbv_core::ws::WsEvent>();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel::<SessionEvent>();
        let (card_image_tx, card_image_rx) =
            mpsc::channel::<(String, Option<image::DynamicImage>)>();
        let (notif_action_tx, notif_action_rx) = mpsc::channel::<String>();
        let (search_tx, search_rx) = mpsc::channel::<(String, Result<Vec<EmbyItem>, String>)>();
        let ui_config = crate::config::load_ui_config().unwrap_or_default();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let library_routes = client.config.library_routes.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let always_play_next = client.config.always_play_next;
        let image_protocol = ui_config.image_protocol.clone();
        let image_protocol_enabled = image_protocol.is_some();
        let image_cache_size = ui_config.image_cache_size;
        let use_nerd_fonts = ui_config.use_nerd_fonts;
        let indicator_style: render::indicators::IndicatorStyle =
            ui_config.indicator_style.parse().unwrap_or_default();
        crate::config::evict_old_image_cache();
        let client_arc = Arc::new(Mutex::new(client));
        {
            let c = client_arc.clone();
            std::thread::spawn(move || {
                let mut probe = c.lock().unwrap().clone();
                probe.probe_chapter_api();
                c.lock().unwrap().chapter_api_available = probe.chapter_api_available;
            });
        }
        let remote_items = remote.items.lock().unwrap().clone();
        let remote_cursor = remote.status.lock().unwrap().current_idx;
        let remote_queue_source = remote.queue_source.lock().unwrap().clone();
        let initial_queue_scope = if !endpoint.is_local() && !remote_items.is_empty() {
            QueueScope::Remote
        } else {
            QueueScope::Local
        };
        let local_daemon_bootstrap = endpoint.is_local().then(|| {
            bootstrap_local_daemon_queue(
                remote_items.clone(),
                remote_cursor,
                remote_queue_source.clone(),
                crate::config::load_queue_state(),
            )
        });
        // `adopt_queue` returns false when the ctrl socket is already dead
        // (the command send failed); tracked so construction doesn't
        // silently carry on with a queue the daemon never actually adopted
        // (#119 task 5) — see `handle_failed_local_daemon_adoption` below.
        let local_daemon_adoption_failed = local_daemon_bootstrap
            .as_ref()
            .and_then(|bootstrap| bootstrap.adopt_queue.clone())
            .is_some_and(|(items, cursor, source)| !remote.adopt_queue(items, cursor, source));
        // Start MPRIS against this `RemotePlayer` (#175, previously done in
        // `main.rs::run_remote_app` before this constructor even ran).
        // Moved here so App owns the resulting handle and can `rebind` it
        // later if `switch_to_direct_remote` / `restore_local_mode` swap
        // which target owns playback.
        let mpris_remote = remote.clone();
        let mpris_handle = crate::mpris::start(
            mpris_remote.status.clone(),
            move |cmd| {
                mpris_remote.send_command(cmd);
            },
            Some(remote.disconnected_flag()),
        );
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
                Some(PlayerTab::new(remote_items, remote_cursor)),
            )
        };
        let mut app = Self::build(AppInit {
            client: client_arc,
            player,
            player_rx,
            ws_rx,
            ws_send_tx: None,
            player_tab,
            remote_player_tab,
            initial_queue_scope,
            system_notifications: false,
            image_protocol,
            image_protocol_enabled,
            hidden_libraries,
            library_routes,
            hidden_latest,
            music_levels,
            use_nerd_fonts,
            indicator_style,
            image_cache_size,
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
        app.mpris = Some(mpris_handle);
        app.player_endpoint = Some(endpoint.clone());
        app.home_is_local_daemon = endpoint.is_local();
        app.sync_subtitle_prefs_to_player();
        app.initialize_shared_state();
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
            if !bootstrap.positions.is_empty() {
                app.spawn_enrich_queue_state(bootstrap.positions);
            }
        } else {
            app.queue_source = remote_queue_source;
        }
        if local_daemon_adoption_failed {
            app.handle_failed_local_daemon_adoption();
        }
        if endpoint.is_local() {
            app.try_auto_reconnect();
        }
        app
    }

    /// Routes a local-daemon queue adoption whose command send failed (dead
    /// ctrl socket, see `new_remote`) through the same disconnect handling a
    /// live `PlayerEvent::RemoteDisconnected` uses, instead of silently
    /// continuing to build on optimistic queue state the daemon never
    /// actually received (#119 task 5).
    pub(super) fn handle_failed_local_daemon_adoption(&mut self) {
        self.handle_player_event(PlayerEvent::RemoteDisconnected(
            "local daemon connection lost while restoring the saved queue".to_string(),
        ));
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
}
