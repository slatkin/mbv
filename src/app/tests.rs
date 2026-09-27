use super::*;

mod actions_tests_queue_state_reseat;
mod audiobookshelf_browse_actions_sibling_tests;
mod audiobookshelf_runtime;
mod auto_reconnect;
mod context_actions;
mod context_menu_placement;
mod daemon_bootstrap;
mod feeds;
mod home_latest;
mod library_navigate_reveal;
mod library_position;
mod library_route;
mod lifecycle;
mod music_grouping;
mod narrow_browse_migration;
mod next_up_accept_dispatch;
mod panel_focus;
mod player_event;
mod podcast;
mod queue;
mod reattach;
mod remote_commands;
pub(crate) mod render_fixtures;
mod route_state;
mod routing_matrix;
mod services_settings_lifecycle;
mod session_connect;
mod settings_activation;
mod split_browse_state_browse_level_tests;
pub(crate) mod tick_integration;

use mbv_emby::test_support::make_session;
use mbv_emby_model::test_support::make_item;
use ratatui::backend::TestBackend;

use ratatui::Terminal;

// ── test helpers ─────────────────────────────────────────────────────────

/// Confirm the populated-queue replacement gate the way the shell's confirm
/// modal does, so a gated replacement executes.
pub(crate) fn confirm_replace_queue(app: &mut App) {
    app.apply_confirm_action(
        crate::app::ConfirmAction::ReplacePopulatedQueue,
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('y'),
            crossterm::event::KeyModifiers::NONE,
        ),
    );
}

pub(crate) fn make_items(n: usize) -> Vec<EmbyItem> {
    (0..n)
        .map(|i| {
            let mut item = make_item(&format!("Item {i}"), "Movie");
            item.id = format!("id{i}");
            item
        })
        .collect()
}

pub(crate) fn make_queue_state(items: Vec<EmbyItem>) -> mbv_queue::QueueState {
    mbv_queue::QueueState::from_emby_items(items, 0, mbv_queue::QueueSource::Unknown)
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

/// Minimal App stub for logic-only tests: the ordinary `App::build`
/// construction (`make_built_app`) plus the stub-friendly defaults below.
pub(crate) fn make_app_stub() -> App {
    let mut app = make_built_app();
    // `build` doubles the configured image cache size; the stub keeps its
    // original (undoubled) budget so eviction behaviour is unchanged.
    app.images.cache_size_total = 50;
    // Never start with a session poll already due.
    app.remote.last_session_poll = Instant::now();
    // Ignore any on-disk feed entry state; a stub starts empty.
    app.feed_entry_state = mbv_feed::FeedEntryStore::default();
    // Default to "focused, past grace window" so existing mouse tests dispatch
    // without arming focus explicitly. The refocus guard itself is tested
    // directly in input_music_track_focus_tests.
    app.refocus_at = Some(Instant::now().checked_sub(Duration::from_secs(5)).unwrap());
    app
}

#[test]
fn emby_completion_applies_bootstrap_and_ready_state() {
    let mut app = make_app_stub();
    let client = mbv_emby::EmbyClient::new(crate::config::Config::default());
    let item = make_item("Ready item", "Audio");
    // Completion computes the Continue Watching snapshot; the shell assigns it.
    let content = app
        .apply_emby_completion(crate::app::dispatch::session::service_startup::Completion {
            generation: app.emby_runtime.generation(),
            result: Ok(crate::app::dispatch::session::service_startup::Startup {
                client,
                bootstrap: mbv_emby::EmbyBootstrap {
                    continue_items: vec![item],
                    views: Vec::new(),
                },
                setup: mbv_config::EmbySetup::default(),
            }),
        })
        .expect("Ok startup must bootstrap the Returned Home content");
    assert_eq!(
        app.emby_runtime.state,
        mbv_core::service_runtime::ServiceState::Ready
    );
    assert!(app.emby_runtime.client.is_some());
    assert_eq!(content.continue_items.len(), 1);
    assert!(!content.loading);
}

#[test]
fn stale_emby_completion_does_not_change_runtime_or_home() {
    let mut app = make_app_stub();
    app.emby_runtime.replace_setup();
    let stale_generation = mbv_core::service_runtime::SetupGeneration::default();
    // Home content is Model-owned (task 5.3d): a stale completion returns no
    // snapshot, so the shell leaves `home_content` untouched — the invariance
    // that used to be asserted on `app.home.continue_items` here.
    let content =
        app.apply_emby_completion(crate::app::dispatch::session::service_startup::Completion {
            generation: stale_generation,
            result: Ok(crate::app::dispatch::session::service_startup::Startup {
                client: mbv_emby::EmbyClient::new(crate::config::Config::default()),
                bootstrap: mbv_emby::EmbyBootstrap::default(),
                setup: mbv_config::EmbySetup::default(),
            }),
        });
    assert_eq!(
        app.emby_runtime.state,
        mbv_core::service_runtime::ServiceState::Connecting
    );
    assert!(app.emby_runtime.client.is_none());
    assert!(
        content.is_none(),
        "stale completion must not deliver a Home content snapshot"
    );
}

pub(crate) fn make_built_app() -> App {
    use mbv_ctrl::player::PlayerStatus;
    use mbv_player::PlayerProxy;
    use std::sync::Mutex;

    let status = Arc::new(Mutex::new(PlayerStatus {
        volume_max: 100,
        ..Default::default()
    }));

    let (_, player_rx) = std::sync::mpsc::channel();
    let (_, ws_rx) = std::sync::mpsc::channel();
    let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
    let channels = crate::app::state::runtime_channels::RuntimeChannels::new();

    let player = PlayerProxy::stub(status);

    let config = crate::config::Config::default();

    App::build(AppInit {
        config: Arc::new(Mutex::new(config)),
        emby_runtime: mbv_core::service_runtime::EmbyRuntime::default(),
        audiobookshelf_runtime: mbv_core::service_runtime::AudiobookshelfRuntime::new(false),
        player,
        player_rx,
        ws_rx,
        ws_send_tx: None,
        audiobookshelf_socket_rx: {
            let (_, rx) = std::sync::mpsc::channel();
            rx
        },
        audiobookshelf_socket_tx: None,
        audiobookshelf_socket_generation: None,
        player_tab: PlayerTab::default(),
        remote_player_tab: None,
        initial_queue_scope: QueueScope::Local,
        system_notifications: false,
        image_protocol: None,
        image_protocol_enabled: false,
        hidden_libraries: Vec::new(),
        library_routes: std::collections::BTreeMap::new(),
        music_levels: Vec::new(),
        use_nerd_fonts: false,
        indicator_style: render::indicators::IndicatorStyle::default(),
        image_cache_size: 50,
        visualizer_glyph: crate::config::DEFAULT_VISUALIZER_GLYPH.into(),
        card_image_tx,
        card_image_rx,
        channels,
        idle_feed: None,
    })
}

pub(crate) fn install_test_emby(app: &mut App, config: crate::config::Config) {
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(mbv_emby::EmbyClient::new(config)),
    ));
}

pub(crate) fn make_remote_app_stub(local_items: Vec<EmbyItem>, remote_items: Vec<EmbyItem>) -> App {
    use crate::config::Config;
    use mbv_emby::EmbyClient;

    let (remote, player_rx) = mbv_remote_player::RemotePlayer::stub(remote_items, 0);
    let config = Config::default();
    let mut app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );
    app.player_tab
        .set_items(local_items, app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 0;
    // Default to "focused, past grace window" for mouse tests.
    app.refocus_at = Some(Instant::now().checked_sub(Duration::from_secs(5)).unwrap());
    app
}

pub(crate) fn make_audio_only_remote_app_stub_with_cmd_rx(
    local_items: Vec<EmbyItem>,
    remote_items: Vec<EmbyItem>,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    use crate::config::Config;
    use mbv_emby::EmbyClient;

    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_audio_only_with_command_rx(remote_items, 0);
    let config = Config::default();
    let mut app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );
    app.player_tab
        .set_items(local_items, app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 0;
    while cmd_rx.try_recv().is_ok() {}
    app.refocus_at = Some(Instant::now().checked_sub(Duration::from_secs(5)).unwrap());
    (app, cmd_rx)
}

pub(crate) fn make_remote_app_stub_with_cmd_rx(
    local_items: Vec<EmbyItem>,
    remote_items: Vec<EmbyItem>,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    use crate::config::Config;
    use mbv_emby::EmbyClient;

    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(remote_items, 0);
    let config = Config::default();
    let mut app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );
    app.player_tab
        .set_items(local_items, app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 0;
    // `App::new_remote` synchronizes this client's subtitle/audio-language
    // prefs to the freshly attached daemon before returning; drain that so
    // callers see only commands their own test actions send.
    while cmd_rx.try_recv().is_ok() {}
    // Default to "focused, past grace window" for mouse tests.
    app.refocus_at = Some(Instant::now().checked_sub(Duration::from_secs(5)).unwrap());
    (app, cmd_rx)
}

pub(crate) fn make_local_daemon_app_stub(remote_items: Vec<EmbyItem>) -> App {
    make_local_daemon_app_stub_with_cmd_rx(remote_items).0
}

pub(crate) fn make_local_daemon_app_stub_with_cmd_rx(
    remote_items: Vec<EmbyItem>,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    use crate::config::Config;
    use mbv_emby::EmbyClient;

    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(remote_items, 0);
    let config = Config {
        stay_alive: true,
        ..Default::default()
    };
    // A local-daemon stub is always stay-alive: tests that model this
    // path must never send RequestShutdown to the real daemon socket.
    let app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Local,
        config,
    );
    (app, cmd_rx)
}

// ── cursor preservation during home refresh ──────────────────────────────

/// Builds a `UnifiedQueueStateData` holding `items` as Emby slots, with slot
/// `active_index` marked active — the shape a daemon broadcast after a queue
/// mutation (cursor follows the active slot; pending client cursors override).
pub(crate) fn emby_unified_state(
    items: &[EmbyItem],
    active_index: usize,
) -> mbv_ctrl::UnifiedQueueStateData {
    let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = items
        .iter()
        .enumerate()
        .map(|(i, item)| mbv_ctrl::UnifiedQueueSlot {
            slot_id: (100 + i) as u64,
            item: mbv_queue::QueueItem::Emby(Box::new(item.clone())),
        })
        .collect();
    mbv_ctrl::UnifiedQueueStateData {
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: slots.get(active_index).map(|s| s.slot_id),
        slots,
        revision: 1,
        source: mbv_queue::QueueSource::Remote,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    }
}
