use super::*;

mod audiobookshelf_book_activation_tests;
mod audiobookshelf_browse_actions_sibling_tests;
mod audiobookshelf_context_menu;
mod audiobookshelf_runtime;
mod auto_reconnect;
mod context_actions;
mod context_menu_entries;
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
mod panel_focus;
mod pinned_view_toggle;
mod podcast;
mod queue;
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
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
    );
}

pub(crate) trait QueueViewTestExt {
    fn adopt_items(&mut self, items: Vec<EmbyItem>, cursor: usize);
    fn emby_items(&self) -> Vec<EmbyItem>;
    fn adopt_queue_items(&mut self, items: Vec<mbv_queue::QueueItem>, cursor: usize);
    fn adopt_queue_items_with_active(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        cursor: usize,
        active_index: usize,
    );
    fn adopt_source(&mut self, source: mbv_queue::QueueSource);
}

impl QueueViewTestExt for QueueView {
    fn adopt_items(&mut self, items: Vec<EmbyItem>, cursor: usize) {
        adopt_queue_view(
            self,
            items
                .into_iter()
                .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor,
            None,
            mbv_queue::QueueSource::Unknown,
        );
    }

    fn emby_items(&self) -> Vec<EmbyItem> {
        self.slots()
            .iter()
            .filter_map(|slot| slot.item.as_emby().cloned())
            .collect()
    }

    fn adopt_queue_items(&mut self, items: Vec<mbv_queue::QueueItem>, cursor: usize) {
        adopt_queue_view(self, items, cursor, None, mbv_queue::QueueSource::Unknown);
    }

    fn adopt_queue_items_with_active(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        cursor: usize,
        active_index: usize,
    ) {
        adopt_queue_view(
            self,
            items,
            cursor,
            Some(active_index),
            mbv_queue::QueueSource::Unknown,
        );
    }

    fn adopt_source(&mut self, source: mbv_queue::QueueSource) {
        adopt_queue_view(
            self,
            self.slots().iter().map(|slot| slot.item.clone()).collect(),
            self.cursor(),
            None,
            source,
        );
    }
}

fn adopt_queue_view(
    view: &mut QueueView,
    items: Vec<mbv_queue::QueueItem>,
    cursor: usize,
    active_index: Option<usize>,
    source: mbv_queue::QueueSource,
) {
    let slots: Vec<_> = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
            slot_id: index as u64 + 1,
            item,
        })
        .collect();
    let state = mbv_ctrl::UnifiedQueueStateData {
        status: mbv_ctrl::player::PlayerStatus {
            current_idx: active_index.unwrap_or_default(),
            ..Default::default()
        },
        active_slot: active_index.and_then(|index| slots.get(index).map(|slot| slot.slot_id)),
        slots,
        revision: 1,
        source,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    view.adopt(
        &state,
        crate::app::state::queue_view::AdoptCause::Replacement,
    );
    view.set_cursor(cursor);
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
    apply_app_stub_defaults(&mut app);
    app
}

fn apply_app_stub_defaults(app: &mut App) {
    // `build` doubles the configured image cache size; the stub keeps its
    // original (undoubled) budget so eviction behaviour is unchanged.
    app.images.set_cache_capacity_for_test(50);
    // Never start with a session poll already due.
    app.remote.last_session_poll = Instant::now();
    // Ignore any on-disk feed entry state; a stub starts empty.
    app.feed_entry_state = mbv_feed::FeedEntryStore::default();
    // Default to "focused, past grace window" so existing mouse tests dispatch
    // without arming focus explicitly. The refocus guard itself is tested
    // directly in input_music_track_focus_tests.
    app.refocus_at = Some(Instant::now().checked_sub(Duration::from_secs(5)).unwrap());
}

/// Re-anchor a stub app onto a fresh remote stub with a live command
/// channel, returning the receiver the test must hold so owner-bound
/// commands report success instead of a dropped-peer disconnect. Unit 4
/// deleted Bare ownership, so `make_app_stub`'s player is a
/// `RemotePlayer::stub` whose command channel is already dropped.
pub(crate) fn live_owner_channel(app: &mut App) -> std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd> {
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.update_status(|status| status.volume_max = 100);
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    app.player_rx = player_rx;
    cmd_rx
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
    app.emby_runtime.begin_setup();
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
    use mbv_player::PlayerProxy;
    use std::sync::Mutex;

    let (remote, player_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);
    remote.update_status(|status| status.volume_max = 100);
    let (_, ws_rx) = std::sync::mpsc::channel();
    let (_, transport_rx) = std::sync::mpsc::channel();
    let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
    let channels = crate::app::state::runtime_channels::RuntimeChannels::new();

    let player = PlayerProxy::from_remote(remote, false);

    let config = crate::config::Config::default();

    App::build(AppInit {
        config: Arc::new(Mutex::new(config)),
        emby_runtime: crate::app::state::service_runtime::EmbyRuntime::default(),
        audiobookshelf_runtime: crate::app::state::service_runtime::AudiobookshelfRuntime::new(
            false,
        ),
        player,
        player_rx,
        ws_rx,
        transport_rx,
        ws_send_tx: None,
        audiobookshelf_socket_rx: {
            let (_, rx) = std::sync::mpsc::channel();
            rx
        },
        audiobookshelf_socket_tx: None,
        audiobookshelf_socket_generation: None,
        local_view: QueueView::default(),
        remote_view: None,
        initial_queue_scope: QueueScope::Local,
        system_notifications: false,
        image_protocol: None,
        image_protocol_enabled: false,
        hidden_libraries: Vec::new(),
        library_routes: std::collections::BTreeMap::new(),
        music_levels: Vec::new(),
        use_nerd_fonts: false,
        indicator_style: mbv_render::indicators::IndicatorStyle::default(),
        image_cache_size: 50,
        visualizer_glyph: crate::config::DEFAULT_VISUALIZER_GLYPH.into(),
        card_image_tx,
        card_image_rx,
        channels,
        idle_feed: None,
    })
}

pub(crate) fn install_test_emby(app: &mut App, config: crate::config::Config) {
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(mbv_emby::EmbyClient::new(config)),
    ));
}

fn remote_stub_config() -> crate::config::Config {
    crate::config::Config::default()
}

fn close_initial_services(app: &mut App) {
    app.close_settings();
    app.pending_overlay = None;
}

pub(crate) fn make_remote_app_stub(local_items: Vec<EmbyItem>, remote_items: Vec<EmbyItem>) -> App {
    make_remote_app_stub_at_index(local_items, remote_items, 0)
}

pub(crate) fn make_remote_app_stub_at_index(
    local_items: Vec<EmbyItem>,
    remote_items: Vec<EmbyItem>,
    current_idx: usize,
) -> App {
    use mbv_emby::EmbyClient;

    let (remote, player_rx) = mbv_remote_player::RemotePlayer::stub(remote_items, current_idx);
    let config = remote_stub_config();
    let mut app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );
    close_initial_services(&mut app);
    app.local_view
        .adopt_items(local_items, app.local_view.cursor());
    app.local_view.set_cursor(0);
    // Default to "focused, past grace window" for mouse tests.
    app.refocus_at = Some(Instant::now().checked_sub(Duration::from_secs(5)).unwrap());
    app
}

pub(crate) fn make_audio_only_remote_app_stub_with_cmd_rx(
    local_items: Vec<EmbyItem>,
    remote_items: Vec<EmbyItem>,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    use mbv_emby::EmbyClient;

    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_audio_only_with_command_rx(remote_items, 0);
    let config = remote_stub_config();
    let mut app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );
    close_initial_services(&mut app);
    app.local_view
        .adopt_items(local_items, app.local_view.cursor());
    app.local_view.set_cursor(0);
    while cmd_rx.try_recv().is_ok() {}
    app.refocus_at = Some(Instant::now().checked_sub(Duration::from_secs(5)).unwrap());
    (app, cmd_rx)
}

pub(crate) fn make_remote_app_stub_with_cmd_rx(
    local_items: Vec<EmbyItem>,
    remote_items: Vec<EmbyItem>,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    use mbv_emby::EmbyClient;

    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(remote_items, 0);
    let config = remote_stub_config();
    let mut app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );
    close_initial_services(&mut app);
    app.local_view
        .adopt_items(local_items, app.local_view.cursor());
    app.local_view.set_cursor(0);
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
        ..remote_stub_config()
    };
    // A local-daemon stub is always stay-alive: tests that model this
    // path must never send RequestShutdown to the real daemon socket.
    let mut app = App::new_remote_with_config(
        EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Local,
        config,
    );
    close_initial_services(&mut app);
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
