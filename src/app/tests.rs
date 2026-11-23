use super::*;

use crate::app::gui_library::GuiLibraryStores;

/// Apply one GUI Library command to a stub `App` a test holds: the test
/// helper for the command tests, which run the command through the GUI core
/// over the `App`, as a client does.
pub(in crate::app) fn apply_gui_library_command(
    app: &mut crate::app::App,
    command: crate::app::GuiLibraryCommand,
) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.apply_gui_library_command(command);
    *app = core.into_app();
}

/// One GUI core step over a stub `App` a test holds: the test helper for the
/// launch tests, which run the launch location's phases through the GUI core
/// over the `App`, as a client does.
pub(in crate::app) fn restore_launch_collections(
    app: &mut crate::app::App,
    state: &mbv_config::GuiLaunchState,
    destination: &mbv_config::TabIdentity,
) -> bool {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    let restored = core.restore_gui_launch_collections(state, destination);
    *app = core.into_app();
    restored
}

/// The launch location's first phase over a stub `App` a test holds.
pub(in crate::app) fn open_launch_destination(
    app: &mut crate::app::App,
    state: &mbv_config::GuiLaunchState,
) -> crate::app::GuiLaunchOpen {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    let opened = core.open_gui_launch_destination(state);
    *app = core.into_app();
    opened
}

/// Run one GUI Library destination's ordered load pass over a stub `App` a
/// test holds: the test helper for the destination load tests, which run the
/// pass through the GUI core over the `App`, as a client does.
pub(in crate::app) fn ensure_gui_destination(
    app: &mut crate::app::App,
    kind: crate::app::GuiDestinationKind,
) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.ensure_gui_destination(kind);
    *app = core.into_app();
}

/// One destination's projection over a stub `App` a test holds.
pub(in crate::app) fn gui_audiobooks_projection(
    app: &mut crate::app::App,
    lib_idx: usize,
) -> crate::app::GuiAudiobooksProjection {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    let projection = core.gui_audiobooks_projection(lib_idx);
    *app = core.into_app();
    projection
}

/// One destination's projection over a stub `App` a test holds.
pub(in crate::app) fn gui_emby_projection(
    app: &mut crate::app::App,
    lib_idx: usize,
) -> crate::app::GuiEmbyProjection {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    let projection = core.gui_emby_projection(lib_idx);
    *app = core.into_app();
    projection
}

/// One destination's projection over a stub `App` a test holds.
pub(in crate::app) fn gui_shows_projection(
    app: &mut crate::app::App,
    lib_idx: usize,
) -> crate::app::GuiShowsProjection {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    let projection = core.gui_shows_projection(lib_idx);
    *app = core.into_app();
    projection
}

/// One destination's Podcasts projection over a stub `App` a test holds.
pub(in crate::app) fn gui_podcasts_projection(
    app: &mut crate::app::App,
    lib_idx: usize,
) -> crate::app::GuiPodcastsProjection {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    let projection = core.gui_podcasts_projection(lib_idx);
    *app = core.into_app();
    projection
}

/// Select one Feeds entry over a stub `App` a test holds.
pub(in crate::app) fn gui_feeds_select(app: &mut crate::app::App, entry: &str) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_feeds_select(entry);
    *app = core.into_app();
}

/// The Feeds projection over a stub `App` a test holds.
pub(in crate::app) fn gui_feeds_projection(
    app: &crate::app::App,
) -> crate::app::GuiFeedsProjection {
    crate::app::gui_library::feeds_projection(app)
}

/// The entry the Feeds Hero shows over a stub `App` a test holds.
pub(in crate::app) fn gui_feeds_picked_entry(app: &crate::app::App) -> Option<&str> {
    crate::app::gui_library::feeds_picked_entry(app)
}

/// The entries the Feeds selection lists over a stub `App` a test holds.
pub(in crate::app) fn gui_feeds_entries(app: &crate::app::App) -> Vec<&mbv_queue::FeedEntry> {
    crate::app::gui_library::feeds_entries(app)
}

/// The Feeds effective set over a stub `App` a test holds.
pub(in crate::app) fn gui_feeds_effective_set(
    app: &crate::app::App,
) -> std::borrow::Cow<'_, crate::app::GuiCollectionSet> {
    crate::app::gui_library::feeds_effective_set(app)
}

/// Open one Shows library over a stub `App` a test holds.
pub(in crate::app) fn gui_tv_open(app: &mut crate::app::App, lib_idx: usize) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_tv_open(lib_idx);
    *app = core.into_app();
}

/// Select one Shows series over a stub `App` a test holds.
pub(in crate::app) fn gui_tv_select_series(app: &mut crate::app::App, lib_idx: usize, id: &str) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_tv_select_series(lib_idx, id);
    *app = core.into_app();
}

/// Select one Shows season over a stub `App` a test holds.
pub(in crate::app) fn gui_tv_select_season(app: &mut crate::app::App, lib_idx: usize, id: &str) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_tv_select_season(lib_idx, id);
    *app = core.into_app();
}

/// One library's Shows view over a stub `App` a test holds.
pub(in crate::app) fn gui_shows_view(
    app: &crate::app::App,
    lib_idx: usize,
) -> crate::app::GuiShowsView {
    crate::app::gui_library::shows_view(app, lib_idx)
}

/// Land one Shows New fetch over a stub `App` a test holds.
pub(in crate::app) fn handle_gui_shows_new(
    app: &mut crate::app::App,
    event: crate::app::gui_library::GuiShowsNewEvent,
) {
    crate::app::gui_library::handle_shows_new(app, event);
}

/// Open one plain Emby library over a stub `App` a test holds.
pub(in crate::app) fn gui_emby_open(app: &mut crate::app::App, lib_idx: usize) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_emby_open(lib_idx);
    *app = core.into_app();
}

/// Select one plain Emby library row over a stub `App` a test holds.
pub(in crate::app) fn gui_emby_select(app: &mut crate::app::App, lib_idx: usize, id: &str) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_emby_select(lib_idx, id);
    *app = core.into_app();
}

/// Open one Audiobooks library over a stub `App` a test holds.
pub(in crate::app) fn gui_abs_open(app: &mut crate::app::App, lib_idx: usize) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_abs_open(lib_idx);
    *app = core.into_app();
}

/// Select one Audiobooks book over a stub `App` a test holds.
pub(in crate::app) fn gui_abs_select(app: &mut crate::app::App, lib_idx: usize, id: &str) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_abs_select(lib_idx, id);
    *app = core.into_app();
}

/// Open one Podcasts destination over a stub `App` a test holds.
pub(in crate::app) fn gui_podcast_open(app: &mut crate::app::App, lib_idx: usize) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_podcast_open(lib_idx);
    *app = core.into_app();
}

/// Select one Podcasts episode over a stub `App` a test holds.
pub(in crate::app) fn gui_podcast_select(
    app: &mut crate::app::App,
    lib_idx: usize,
    target: &mbv_ui_msg::PodcastEpisodeTarget,
) {
    let row = crate::app::gui_library::GuiPodcastEpisodeId::from(target).encode();
    apply_gui_library_command(
        app,
        crate::app::GuiLibraryCommand::PodcastsSelect { lib_idx, row },
    );
}

/// The right-click row menu over a stub `App` a test holds.
pub(in crate::app) fn gui_library_row_menu(
    app: &mut crate::app::App,
) -> Option<crate::app::GuiLibraryRowMenu> {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    let menu = core.gui_library_row_menu();
    *app = core.into_app();
    menu
}

/// Queue the in-view row fetches over a stub `App` a test holds.
pub(in crate::app) fn gui_queue_in_view_row_fetches(app: &mut crate::app::App) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.gui_queue_in_view_row_fetches();
    *app = core.into_app();
}

/// Whether an interrupted art retry came due, over a stub `App` a test holds.
pub(in crate::app) fn gui_art_retry_due(
    app: &crate::app::App,
    after: std::time::Instant,
    until: std::time::Instant,
) -> bool {
    crate::app::gui_library::marks::art_retry_due(app, after, until)
}

// The terminal-size helpers (`set_terminal_size`, `set_terminal_width`) moved
// to `crate::app::shell::test_support` (task 4.7): they write the shell
// `Model`'s size and push the single-panel fact the core reads.

mod audiobookshelf_book_activation_tests;
mod audiobookshelf_browse_actions_sibling_tests;
mod audiobookshelf_context_menu;
mod audiobookshelf_mark;
mod audiobookshelf_runtime;
mod auto_reconnect;
mod browse_cache_caps_tests;
mod context_actions;
mod context_menu_entries;
mod context_menu_placement;
mod daemon_bootstrap;
mod feeds;
mod gui_client_settings;
mod gui_library;
mod gui_library_audiobooks;
mod gui_library_emby;
mod gui_library_ensure;
mod gui_library_feeds;
mod gui_library_home;
mod gui_library_podcasts;
mod gui_library_rows;
mod gui_library_search;
mod gui_library_shows;
mod gui_library_tv;
mod gui_playlists;
mod gui_settings;
mod home_latest;
mod library_navigate_reveal;
mod library_position;
mod library_route;
mod library_viewport_reach_tests;
mod lifecycle;
mod music_grouping;
mod named_queue_controls;
mod named_queues;
mod narrow_browse_migration;
mod output_handoff;
mod panel_focus;
mod pinned_view_toggle;
mod podcast;
mod queue;
pub(crate) mod render_fixtures;
mod route_state;
mod routing_matrix;
mod services_settings_lifecycle;
mod session_connect;
mod settings_activation;
mod split_browse_state_browse_level_tests;
pub(crate) mod tick_integration;
mod twitch_url;

use mbv_emby::test_support::make_session;
// `TestBackend`, `Terminal`, and `confirm_replace_queue` moved to
// `crate::app::shell::test_support` (task 4.6c): they name the toolkit.
use mbv_emby_model::test_support::make_item;

// The App stub construction moved to `app_stub` so the Clients' own tests
// can build it too; the `crate::app::tests::*` paths stay working.
pub(crate) use super::app_stub::built::make_built_app;
pub(crate) use super::app_stub::{gui_client_capabilities, make_app_stub};

// ── test helpers ─────────────────────────────────────────────────────────

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
        queue: None,
        start_slot: None,
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
    // Home content lives on the App (row 6.1): a stale completion returns no
    // snapshot, so the stored `home_content` stays untouched — the invariance
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
    make_local_daemon_app_stub_with_cmd_rx_and_compat(
        remote_items,
        mbv_ctrl::CtrlCompatibility::current(),
    )
}

/// Variant whose Owner advertises `compat` instead of the stub's all-off
/// capability baseline — for flows that gate on or use an owner capability
/// a real Owner advertises from its Hello handshake.
pub(crate) fn make_local_daemon_app_stub_with_cmd_rx_and_compat(
    remote_items: Vec<EmbyItem>,
    compat: mbv_ctrl::CtrlCompatibility,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    use crate::config::Config;
    use mbv_emby::EmbyClient;

    let (mut remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(remote_items, 0);
    remote.set_ctrl_compatibility_for_test(compat);
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
        queue: None,
        start_slot: None,
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
