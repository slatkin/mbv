use crate::app::state::app_struct::LaunchRestore;
use crate::app::state::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use crate::app::{App, AppInit, spawn_resize_worker};
use mbv_render::layout;
use mbv_render::layout::LEFT_WIDTH_DEFAULT;
use mbv_ui_model::settings::{PanelFocus, PanelMode};
use mbv_ui_model::tab_selection::TabSelection;
use std::sync::mpsc;
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

/// Load the legacy per-library position document with its pill fields
/// cleared. The document is a startup migration reader only — it is never
/// written — so its letter-filter, TV content mode, and feed-group fields
/// would otherwise resurrect a pill from an old run forever. Pill choices
/// are session memory; restart always resolves the destination's default.
fn load_position_state_without_pills() -> mbv_queue::LibraryPositionState {
    let mut state = crate::config::load_library_position_state();
    for position in state.libraries.values_mut() {
        position.feed_selected_group = 0;
        for level in &mut position.levels {
            level.letter_filter_index = None;
            level.tv_content_mode = None;
        }
    }
    state
}

fn independent_emby_runtime(configured: bool, credential_present: bool) -> EmbyRuntime {
    let mut runtime = EmbyRuntime::new(configured);
    runtime.state = crate::app::dispatch::session::service_startup::initial_state(
        configured,
        credential_present,
    );
    runtime
}

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

fn detached_socket_rx() -> mpsc::Receiver<mbv_audiobookshelf::socket::SocketEvent> {
    let (_, rx) = mpsc::channel();
    rx
}

impl App {
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
        let launch_restore =
            mbv_config::load_tui_launch_state().map_or(LaunchRestore::Done, LaunchRestore::Pending);
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
            mpris: None,
            player_rx: init.player_rx,
            deferred_player_events: std::collections::VecDeque::new(),
            deferred_home_events: std::collections::VecDeque::new(),
            ws_rx: init.ws_rx,
            transport_rx: init.transport_rx,
            ws_send_tx: init.ws_send_tx,
            audiobookshelf_socket_rx: init.audiobookshelf_socket_rx,
            audiobookshelf_socket_tx: init.audiobookshelf_socket_tx,
            audiobookshelf_socket_generation: init.audiobookshelf_socket_generation,
            local_view: init.local_view,
            remote_view: init.remote_view,
            system_notifications: init.system_notifications,
            images: mbv_images::cache::ImageCache::new(
                init.image_cache_size,
                init.image_protocol,
                init.image_protocol_enabled,
                init.card_image_tx,
                init.card_image_rx,
                resize_register_tx,
                resize_response_rx,
            ),
            library_position_state: load_position_state_without_pills(),
            hidden_libraries: init.hidden_libraries,
            library_routes: init.library_routes,
            home_latest_launch_window: mbv_ui_model::home_latest::HomeLatestLaunchWindow {
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
            pinned_panel: None,
            pinned_width: crate::pin::PinnedWidth::default(),
            accent_custom: None,
            pinned_resize_pending: false,
            resize_drag_activity: None,

            pending_overlay: None,
            pending_exit_message: None,
            pending_delete_slot: None,
            queue_undo_stack: Vec::new(),
            remote_queue_undo_stack: Vec::new(),
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
            // Always start on Home until the saved launch tab settles.
            launch_restore,
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
            queue_card_projection: mbv_ui_model::playback::QueueCardProjection::default(),
            title_log_gate: crate::app::state::projection::card::TitleLogGate::default(),
            dim_backdrop_active: false,
            settings_destination: mbv_ui_model::settings::SettingsDestination::Main,
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
            queue_dirty: false,
            pending_owner_source_update: None,
            queue_deferrals: crate::app::QueueDeferrals::default(),
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
            window_focused: true,
            album_artist_cache: std::collections::HashMap::new(),
            album_artist_levels: std::collections::HashMap::new(),
            pending_level_artist_warmups: std::collections::VecDeque::new(),
            level_artist_warmups_in_flight: std::collections::HashSet::new(),
            album_tracks_cache: std::collections::HashMap::new(),
            album_tracks_cache_order: std::collections::VecDeque::new(),
            album_tracks_loading: std::collections::HashSet::new(),
            pending_artist_album_track_fetches: std::collections::VecDeque::new(),
            artist_album_track_fetches_in_flight: std::collections::HashSet::new(),
            artist_detail_cache: std::collections::HashMap::new(),
            artist_detail_loading: std::collections::HashSet::new(),
            artist_artwork_requests: std::collections::HashMap::new(),
            artist_artwork_status: std::collections::HashMap::new(),
            series_detail_cache: std::collections::HashMap::new(),
            series_detail_cache_order: std::collections::VecDeque::new(),
            series_detail_loading: std::collections::HashSet::new(),
            series_season_loading: std::collections::HashSet::new(),
            pending_series_season_expansions: std::collections::HashSet::new(),
            queue_scope: init.initial_queue_scope,
            launched_as_remote: false,
            player_endpoint: None,
            home_is_local_daemon: false,
            idle_feed: init.idle_feed,
            feed_seek_pending_slot: None,
            feed_tab: mbv_ui_model::feed_tab::FeedTabState::default(),
            feed_entry_state: mbv_feed::FeedEntryStore::load(),
        };
        app.sync_feed_subscriptions();
        app
    }

    /// Initialize terminal image pickers before the TUI listener starts.
    pub(crate) fn init_image_pickers(&mut self) {
        self.images.init_image_pickers();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use mbv_queue::{LibraryPosition, LibraryPositionLevel, LibraryPositionState, TvContentMode};

    use super::load_position_state_without_pills;

    /// Owns the spec rule that the legacy position document cannot
    /// resurrect a pill: its letter-filter, TV mode, and feed-group fields
    /// are cleared at load, while the rest of the position survives.
    #[test]
    fn loading_the_legacy_position_document_clears_its_pill_fields() {
        let _guard = crate::config::TestStateDirGuard::new();
        mbv_config::save_library_position_state(&LibraryPositionState {
            libraries: HashMap::from([(
                "lib-movies".to_string(),
                LibraryPosition {
                    feed_selected_group: 2,
                    levels: vec![LibraryPositionLevel {
                        focused_item_id: Some("kept-item".into()),
                        cursor_index: 4,
                        letter_filter_index: Some(3),
                        tv_content_mode: Some(TvContentMode::Range(2)),
                        ..LibraryPositionLevel::default()
                    }],
                    ..LibraryPosition::default()
                },
            )]),
        });

        let loaded = load_position_state_without_pills();
        let position = loaded.libraries.get("lib-movies").expect("entry");
        assert_eq!(position.feed_selected_group, 0);
        let level = &position.levels[0];
        assert_eq!(level.letter_filter_index, None);
        assert_eq!(level.tv_content_mode, None);
        assert_eq!(level.focused_item_id.as_deref(), Some("kept-item"));
        assert_eq!(level.cursor_index, 4);
    }
}
