use super::types_settings::SettingsDestination;
use super::*;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use ratatui::backend::TestBackend;

use ratatui::Terminal;

pub(crate) fn make_item(name: &str, item_type: &str) -> EmbyItem {
    EmbyItem {
        id: "id".into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 0,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        director: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genre: String::new(),
        playlist_item_id: String::new(),
    }
}

pub(crate) fn make_session(device_name: &str, client: &str) -> mbv_core::api::SessionInfo {
    mbv_core::api::SessionInfo {
        id: "sess-1".into(),
        device_name: device_name.into(),
        client: client.into(),
        user_name: "user".into(),
        host: "127.0.0.1".into(),
        supported_commands: Vec::new(),
        now_playing: None,
        now_playing_item_id: None,
        position_s: 0,
        runtime_s: 0,
        position_ticks: 0,
        runtime_ticks: 0,
        is_paused: false,
        volume: 100,
        sub_index: -1,
        audio_index: 1,
        muted: false,
        media_info: mbv_core::api::SessionMediaInfo::default(),
    }
}

/// Minimal daemon-side protocol handshake for tests that need a real
/// TCP socket `RemotePlayer::connect_endpoint` can connect to (#233):
/// sends the protocol hello, drains the client's hello line, then
/// sends an empty initial state. Returns the accepted `TcpStream` so
/// the caller can observe what happens to it afterward (e.g. that the
/// client shuts it down).
pub(crate) fn run_stub_daemon_handshake(stream: std::net::TcpStream) -> std::net::TcpStream {
    use std::io::{BufRead, BufReader, Write};
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    let hello = serde_json::to_string(&mbv_core::ctrl::CtrlEvent::Hello(
        mbv_core::ctrl::CtrlHello::current(),
    ))
    .unwrap();
    writeln!(writer, "{hello}").unwrap();

    let mut client_hello = String::new();
    reader.read_line(&mut client_hello).unwrap();

    let initial_state = serde_json::to_string(&mbv_core::ctrl::CtrlEvent::State(
        mbv_core::ctrl::CtrlState {
            status: mbv_core::player::PlayerStatus::default(),
            items: Vec::new(),
            cursor: 0,
            source: crate::config::QueueSource::Unknown,
            feed_items: Vec::new(),
        },
    ))
    .unwrap();
    writeln!(writer, "{initial_state}").unwrap();

    stream
}

// ── test helpers ─────────────────────────────────────────────────────────

pub(crate) fn make_items(n: usize) -> Vec<EmbyItem> {
    (0..n)
        .map(|i| {
            let mut item = make_item(&format!("Item {i}"), "Movie");
            item.id = format!("id{i}");
            item
        })
        .collect()
}

pub(crate) fn make_queue_state(items: Vec<EmbyItem>) -> crate::config::QueueState {
    crate::config::QueueState::from_emby_items(items, 0, crate::config::QueueSource::Unknown)
}

pub(crate) fn make_audio_items(n: usize) -> Vec<EmbyItem> {
    (0..n)
        .map(|i| {
            let mut item = make_item(&format!("Track {i}"), "Audio");
            item.id = format!("id{i}");
            item.media_type = "Audio".into();
            item
        })
        .collect()
}

/// Minimal App stub for logic-only tests.
pub(crate) fn make_app_stub() -> App {
    use mbv_core::player::{PlayerProxy, PlayerStatus};
    use std::sync::{Arc, Mutex};

    let status = Arc::new(Mutex::new(PlayerStatus {
        volume_max: 100,
        ..Default::default()
    }));

    let (_, player_rx) = std::sync::mpsc::channel();
    let (_, ws_rx) = std::sync::mpsc::channel();
    let (lib_tx, lib_rx) = std::sync::mpsc::channel();
    let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
    // No worker thread spawned here: `image_picker` is always `None` in
    // this stub, so no `ThreadProtocol` is ever built and nothing sends
    // on `resize_register_tx`/reads `resize_response_rx`.
    let (resize_register_tx, _resize_register_rx) = std::sync::mpsc::channel();
    let (_resize_response_tx, resize_response_rx) = std::sync::mpsc::channel();
    let (notif_action_tx, notif_action_rx) = std::sync::mpsc::channel::<String>();
    let (sessions_tx, sessions_rx) = std::sync::mpsc::channel();
    let (search_tx, search_rx) =
        std::sync::mpsc::channel::<(String, Result<Vec<EmbyItem>, String>)>();

    let player = PlayerProxy::stub(status.clone());

    use crate::config::Config;
    let config = Config::default();

    App {
        _test_state_dir_guard: crate::config::TestStateDirGuard::new_if_unset(),
        config: Arc::new(Mutex::new(config)),
        emby_runtime: mbv_core::service_runtime::EmbyRuntime::default(),
        audiobookshelf_runtime: mbv_core::service_runtime::AudiobookshelfRuntime::new(false),
        emby_startup_rx: None,
        emby_startup_request: None,
        audiobookshelf_startup_rx: None,
        audiobookshelf_startup_request: None,
        audiobookshelf_test_rx: None,
        audiobookshelf_setup_rx: None,
        audiobookshelf_catalog_rx: None,
        audiobookshelf_libraries: Vec::new(),
        audiobookshelf_browse: Vec::new(),
        emby_setup_form: None,
        audiobookshelf_setup_form: None,
        emby_setup_rx: None,
        pending_emby_replacement: None,
        pending_audiobookshelf_replacement: None,
        shared_client: None,
        shared_reconnect_rx: None,
        player,
        mpris: None,
        launched_as_remote: false,
        player_rx,
        ws_rx,
        hidden_libraries: Vec::new(),
        library_routes: std::collections::HashMap::new(),
        hidden_latest: Vec::new(),
        music_levels: Vec::new(),
        album_indexes: std::collections::HashMap::new(),
        player_tab: PlayerTab::default(),
        remote_player_tab: None,
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

        home_loading: false,
        mouse_col: 0,
        mouse_row: 0,
        last_click_time: std::time::Instant::now(),
        last_drag_seek: std::time::Instant::now(),
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
        panel_focus: PanelFocus::default(),
        tab: TabSelection::Home,
        queue_column_width: LEFT_WIDTH_DEFAULT,
        panel_mode: PanelMode::default(),
        library_tab_pending: 0,
        queue_scroll: 0,
        last_played_item_id: None,
        last_played_completed: false,
        card_image_states: std::collections::HashMap::new(),
        card_image_loading: std::collections::HashSet::new(),
        last_card_height: 0,
        last_card_width: 0,
        card_image_tx,
        card_image_rx,
        resize_register_tx,
        resize_response_rx,
        image_picker: None,
        halfblock_picker: None,
        dim_backdrop_active: false,
        image_cache_size_total: 50,
        show_help: false,
        show_settings: false,
        settings_cursor: 0,
        settings_destination: SettingsDestination::Main,
        services_cursor: 0,
        settings_scroll: 0,
        settings_save_at: None,
        confirm_logout: false,
        multiselect_popup: None,
        library_routes_popup: None,
        help_scroll: 0,
        system_notifications: false,
        notif_failed: false,
        notif_action_tx,
        notif_action_rx,
        context_menu: None,
        lib_tx,
        lib_rx,
        search_tx,
        search_rx,
        search_debounce_deadline: None,
        search_debounce_pending: None,
        search_sidebar: None,
        force_clear: false,
        tab_scroll: 0,
        ui_volume: 100,
        pre_mute_volume: None,
        mute_on: false,
        visualizer_enabled: false,
        visualizer_failed: false,
        visualizer: None,
        visualizer_frame: Vec::new(),
        now_playing_throbber_index: 0,
        last_throbber_advance: std::time::Instant::now(),
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
        use_nerd_fonts: false,
        indicator_style: Default::default(),
        ws_send_tx: None,
        last_keepalive: Instant::now(),
        last_capabilities: Instant::now(),
        sessions_tx,
        sessions_rx,
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
        last_session_poll: std::time::Instant::now(),
        session_miss_count: 0,
        remote_pos_s: 0,
        remote_pos_at: std::time::Instant::now(),
        remote_api_pos_advanced_at: std::time::Instant::now() - Duration::from_secs(60),
        remote_seek_pending_until: std::time::Instant::now() - Duration::from_secs(1),
        runtime_zero_since: None,
        suspended_local: None,
        active_route: None,
        library_route_cache: std::collections::HashMap::new(),
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
        image_cache_size: 50,
        image_protocol: None,
        image_protocol_enabled: false,
        library_position_state: crate::config::LibraryPositionState::default(),
        queue_scope: QueueScope::Local,
        player_endpoint: None,
        home_is_local_daemon: false,
        idle_feed: None,
        feed_seek_pending_slot: None,
        feed_tab: super::types_feed_tab::FeedTabState::default(),
        feeds_manage_popup: None,
    }
}

#[test]
fn emby_completion_applies_bootstrap_and_ready_state() {
    let mut app = make_app_stub();
    let client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    let item = make_item("Ready item", "Audio");
    app.apply_emby_completion(super::service_startup::Completion {
        generation: app.emby_runtime.generation(),
        result: Ok(super::service_startup::Startup {
            client,
            bootstrap: mbv_core::service_runtime::EmbyBootstrap {
                continue_items: vec![item],
                views: Vec::new(),
                latest: Vec::new(),
            },
            setup: mbv_core::config::EmbySetup::default(),
        }),
    });
    assert_eq!(
        app.emby_runtime.state,
        mbv_core::service_runtime::ServiceState::Ready
    );
    assert!(app.emby_runtime.client.is_some());
    assert_eq!(app.home.continue_items.len(), 1);
    assert!(!app.home_loading);
}

#[test]
fn emby_completion_classifies_current_failures_without_client() {
    let mut app = make_app_stub();
    app.apply_emby_completion(super::service_startup::Completion {
        generation: app.emby_runtime.generation(),
        result: Err(mbv_core::service_runtime::EmbyFailure {
            class: mbv_core::service_runtime::EmbyFailureClass::AuthenticationRejected,
            message: "HTTP 401 unauthorized".into(),
        }),
    });
    assert_eq!(
        app.emby_runtime.state,
        mbv_core::service_runtime::ServiceState::NeedsAuthentication
    );
    assert!(app.emby_runtime.client.is_none());
    assert!(!app.home_loading);

    let mut app = make_app_stub();
    app.apply_emby_completion(super::service_startup::Completion {
        generation: app.emby_runtime.generation(),
        result: Err(mbv_core::service_runtime::EmbyFailure::unavailable(
            "connection refused",
        )),
    });
    assert_eq!(
        app.emby_runtime.state,
        mbv_core::service_runtime::ServiceState::Unavailable
    );
    assert!(app.emby_runtime.client.is_none());
    assert!(!app.home_loading);
}

#[test]
fn stale_emby_completion_does_not_change_runtime_or_home() {
    let mut app = make_app_stub();
    app.home.continue_items = vec![make_item("current", "Audio")];
    app.emby_runtime.replace_setup();
    let stale_generation = mbv_core::service_runtime::SetupGeneration::default();
    app.apply_emby_completion(super::service_startup::Completion {
        generation: stale_generation,
        result: Ok(super::service_startup::Startup {
            client: mbv_core::api::EmbyClient::new(crate::config::Config::default()),
            bootstrap: mbv_core::service_runtime::EmbyBootstrap::default(),
            setup: mbv_core::config::EmbySetup::default(),
        }),
    });
    assert_eq!(
        app.emby_runtime.state,
        mbv_core::service_runtime::ServiceState::Connecting
    );
    assert!(app.emby_runtime.client.is_none());
    assert_eq!(app.home.continue_items[0].name, "current");
}

pub(crate) fn make_built_app() -> App {
    use mbv_core::player::{PlayerProxy, PlayerStatus};
    use std::sync::{Arc, Mutex};

    let status = Arc::new(Mutex::new(PlayerStatus {
        volume_max: 100,
        ..Default::default()
    }));

    let (_, player_rx) = std::sync::mpsc::channel();
    let (_, ws_rx) = std::sync::mpsc::channel();
    let (lib_tx, lib_rx) = std::sync::mpsc::channel();
    let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
    let (notif_action_tx, notif_action_rx) = std::sync::mpsc::channel::<String>();
    let (sessions_tx, sessions_rx) = std::sync::mpsc::channel();
    let (search_tx, search_rx) =
        std::sync::mpsc::channel::<(String, Result<Vec<EmbyItem>, String>)>();

    let player = PlayerProxy::stub(status);

    use crate::config::Config;
    let config = Config::default();

    App::build(AppInit {
        config: Arc::new(Mutex::new(config)),
        emby_runtime: mbv_core::service_runtime::EmbyRuntime::default(),
        audiobookshelf_runtime: mbv_core::service_runtime::AudiobookshelfRuntime::new(false),
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
        player_tab: PlayerTab::default(),
        remote_player_tab: None,
        initial_queue_scope: QueueScope::Local,
        system_notifications: false,
        image_protocol: None,
        image_protocol_enabled: false,
        hidden_libraries: Vec::new(),
        library_routes: std::collections::HashMap::new(),
        hidden_latest: Vec::new(),
        music_levels: Vec::new(),
        use_nerd_fonts: false,
        indicator_style: render::indicators::IndicatorStyle::default(),
        image_cache_size: 50,
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
    })
}

pub(crate) fn install_test_emby(app: &mut App, config: crate::config::Config) {
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(mbv_core::api::EmbyClient::new(config)),
    ));
}

pub(crate) fn left_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

pub(crate) fn render_app_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();

    let buf = term.backend().buffer();
    let area = *buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

pub(crate) fn make_remote_app_stub(local_items: Vec<EmbyItem>, remote_items: Vec<EmbyItem>) -> App {
    use crate::config::Config;
    use mbv_core::api::EmbyClient;

    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let config = Config::default();
    let mut app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );
    app.player_tab
        .set_items(local_items, app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 0;
    app
}

pub(crate) fn make_remote_app_stub_with_cmd_rx(
    local_items: Vec<EmbyItem>,
    remote_items: Vec<EmbyItem>,
) -> (App, std::sync::mpsc::Receiver<mbv_core::ctrl::CtrlCmd>) {
    use crate::config::Config;
    use mbv_core::api::EmbyClient;

    let (remote, player_rx, cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(remote_items, 0);
    let config = Config::default();
    let mut app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );
    app.player_tab
        .set_items(local_items, app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 0;
    // `App::new_remote` synchronizes this client's subtitle/audio-language
    // prefs to the freshly attached daemon before returning; drain that so
    // callers see only commands their own test actions send.
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

pub(crate) fn make_local_daemon_app_stub(remote_items: Vec<EmbyItem>) -> App {
    use crate::config::Config;
    use mbv_core::api::EmbyClient;

    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let config = Config::default();
    App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Local,
        config,
    )
}

// ── cursor preservation during home refresh ──────────────────────────────

pub(crate) fn sections(n: usize) -> Vec<(String, String, Vec<EmbyItem>, usize)> {
    (0..n)
        .map(|i| (format!("Sec {i}"), format!("lib{i}"), make_items(3), 0))
        .collect()
}
