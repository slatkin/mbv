//! The whole-`App` fixtures a Client's own test builds (`2-split-tui-gui-clients`
//! design D4b rule 5): the stub Owner constructions two Clients share, the item
//! builders, and the queue shapes a menu or transport test reads. They sit in
//! the core so one fixture serves every test that builds it, and they compile
//! under the `test` feature so a dependent Client's `test` build reaches them.

use std::sync::mpsc;
use std::time::Duration;

use crate::app::App;
use crate::app::config::Config;
use mbv_emby_model::EmbyItem;

/// `n` Emby movie stubs with numbered ids, the shape a browse fixture takes.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_items(n: usize) -> Vec<mbv_emby_model::EmbyItem> {
    (0..n)
        .map(|i| {
            let mut item = mbv_emby_model::test_support::make_item(&format!("Item {i}"), "Movie");
            item.id = format!("id{i}");
            item
        })
        .collect()
}

/// `n` Emby audio-track stubs with numbered ids, the shape a browse fixture
/// takes.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_audio_items(n: usize) -> Vec<mbv_emby_model::EmbyItem> {
    (0..n)
        .map(|i| {
            let mut item = mbv_emby_model::test_support::make_item(&format!("Track {i}"), "Audio");
            item.id = format!("id{i}");
            item.media_type = "Audio".into();
            item
        })
        .collect()
}

/// Point the `App`'s active player at an Owner that advertises or hides the
/// `queue-shuffle` ctrl capability (queue-content-shuffle design D6): the
/// Queue list Shuffle entry is offered or hidden from it.
#[cfg(any(test, feature = "test"))]
pub fn seed_queue_shuffle_owner_for_test(app: &mut App, shuffle_capable: bool) {
    let (mut remote, _events, _cmd) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    let mut compat = mbv_ctrl::CtrlCompatibility::current();
    compat.supports_queue_shuffle = shuffle_capable;
    remote.set_ctrl_compatibility_for_test(compat);
    app.seed_active_player_for_test(mbv_player::PlayerProxy::from_remote(remote, false));
}

/// Seed the Queue panel's displayed rows from `items`, with Queue focused: the
/// one-slot fixture a Queue list menu test reads its entries over.
#[cfg(any(test, feature = "test"))]
pub fn seed_queue_row_menu_rows_for_test(app: &mut App, items: Vec<mbv_queue::QueueItem>) {
    app.set_panel_focus_for_test(mbv_ui_model::settings::PanelFocus::Queue);
    app.seed_local_queue_snapshot_for_test(&queue_menu_state(items));
}

/// One remote queue's snapshot holding `items`, its first slot active.
#[cfg(any(test, feature = "test"))]
fn queue_menu_state(items: Vec<mbv_queue::QueueItem>) -> mbv_ctrl::UnifiedQueueStateData {
    mbv_ctrl::UnifiedQueueStateData {
        queue: None,
        start_slot: None,
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: Some(1),
        slots: items
            .into_iter()
            .enumerate()
            .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                slot_id: u64::try_from(index).unwrap_or(u64::MAX) + 1,
                item,
            })
            .collect(),
        revision: 1,
        source: mbv_queue::QueueSource::Remote,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    }
}

/// The `Config` a remote stub `App` is built over: the defaults, so a test
/// reads only what it sets.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn remote_stub_config() -> Config {
    Config::default()
}

/// Close the Services and overlays a construction opens, so a test sees only
/// what its own actions raise.
#[cfg(any(test, feature = "test"))]
pub fn close_initial_services(app: &mut App) {
    app.close_settings();
    app.clear_pending_overlay_for_test();
}

/// The `App` a test drives attached to a local daemon Owner over
/// `remote_items`, whose command channel the test holds.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_local_daemon_app_stub(remote_items: Vec<EmbyItem>) -> App {
    make_local_daemon_app_stub_with_cmd_rx(remote_items).0
}

/// The `App` a test drives attached to a local daemon Owner over
/// `remote_items`, with the Owner's command receiver.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_local_daemon_app_stub_with_cmd_rx(
    remote_items: Vec<EmbyItem>,
) -> (App, mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    make_local_daemon_app_stub_with_cmd_rx_and_compat(
        remote_items,
        mbv_ctrl::CtrlCompatibility::current(),
    )
}

/// Variant whose Owner advertises `compat` instead of the stub's all-off
/// capability baseline — for flows that gate on or use an owner capability
/// a real Owner advertises from its Hello handshake.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_local_daemon_app_stub_with_cmd_rx_and_compat(
    remote_items: Vec<EmbyItem>,
    compat: mbv_ctrl::CtrlCompatibility,
) -> (App, mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
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

/// A `UnifiedQueueStateData` holding `items` as Emby slots, with slot
/// `active_index` marked active — the shape a daemon broadcast after a queue
/// mutation (cursor follows the active slot; pending client cursors override).
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn emby_unified_state(
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

#[cfg(any(test, feature = "test"))]
pub fn install_test_emby(app: &mut App, config: crate::app::config::Config) {
    app.adopt_ready_emby_client_for_test(mbv_emby::EmbyClient::new(config));
}

/// The `App` a test drives attached to a remote stub Owner over
/// `remote_items`, holding `local_items` on its own queue.
///
/// # Panics
/// Panics if the endpoint literal fails to parse: it is a constant, so this
/// never happens.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_remote_app_stub(local_items: Vec<EmbyItem>, remote_items: Vec<EmbyItem>) -> App {
    make_remote_app_stub_at_index(local_items, remote_items, 0)
}

/// The `App` a test drives attached to a remote stub Owner whose playhead
/// rests at `current_idx`, holding `local_items` on its own queue.
///
/// # Panics
/// Panics if the endpoint literal fails to parse: it is a constant, so this
/// never happens.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_remote_app_stub_at_index(
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
    app.seed_local_queue_emby_items_for_test(local_items, 0);
    // Default to "focused, past grace window" for mouse tests.
    app.backdate_refocus_for_test(Duration::from_secs(5));
    app
}

/// The `App` a test drives attached to an audio-only remote stub Owner, with
/// the Owner's command receiver.
///
/// # Panics
/// Panics if the endpoint literal fails to parse: it is a constant, so this
/// never happens.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_audio_only_remote_app_stub_with_cmd_rx(
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
    app.seed_local_queue_emby_items_for_test(local_items, 0);
    while cmd_rx.try_recv().is_ok() {}
    app.backdate_refocus_for_test(Duration::from_secs(5));
    (app, cmd_rx)
}

/// The `App` a test drives attached to a remote stub Owner over
/// `remote_items`, with the Owner's command receiver.
///
/// # Panics
/// Panics if the endpoint literal fails to parse: it is a constant, so this
/// never happens.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_remote_app_stub_with_cmd_rx(
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
    app.seed_local_queue_emby_items_for_test(local_items, 0);
    // `App::new_remote` synchronizes this client's subtitle/audio-language
    // prefs to the freshly attached daemon before returning; drain that so
    // callers see only commands their own test actions send.
    while cmd_rx.try_recv().is_ok() {}
    // Default to "focused, past grace window" for mouse tests.
    app.backdate_refocus_for_test(Duration::from_secs(5));
    (app, cmd_rx)
}

/// Re-anchor a stub app onto a fresh remote stub with a live command
/// channel, returning the receiver the test must hold so owner-bound
/// commands report success instead of a dropped-peer disconnect. Unit 4
/// deleted Bare ownership, so `make_app_stub`'s player is a
/// `RemotePlayer::stub` whose command channel is already dropped.
#[cfg(any(test, feature = "test"))]
pub fn live_owner_channel(app: &mut App) -> std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd> {
    app.set_player_with_command_rx_for_test()
}
