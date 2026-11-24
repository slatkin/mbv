//! A self-contained `App` stub for the Clients' own tests
//! (`extract-client-core` design D11): the ordinary `App::build` construction
//! with stub-friendly defaults, no live Owner, and no live Service. The GUI
//! Client's headless tests build their `App` here, exactly as the TUI
//! Client's tests build theirs in `app::tests`.
//!
//! The module compiles under `cfg(any(test, feature = "test"))`, so a
//! dependent crate's dev-dependency can enable the `test` feature for it;
//! the shipped builds never compile it.

#[cfg(any(test, feature = "test"))]
use std::sync::{Arc, Mutex};

use super::App;
#[cfg(any(test, feature = "test"))]
use super::QueueView;

/// The stub constructions over `App::build`, in their own child module so this
/// one stays within its line ceiling. Their first half is the construction a
/// caller outside the `test` builds uses too (the parked `App` a Client wrapper
/// moves out, and the GUI modules' own paths).
pub(in crate::app) mod built;

/// The whole-`App` fixtures and the item builders two Clients' tests share
/// (`2-split-tui-gui-clients` design D4b rule 5), in their own child module so
/// this file stays within its line ceiling.
#[cfg(any(test, feature = "test"))]
mod fixtures;

/// The Named-queue fixtures a Client's own test builds (`2-split-tui-gui-clients`
/// design D4b rule 5), in their own child module for the same reason.
#[cfg(any(test, feature = "test"))]
mod named_queues;

#[cfg(any(test, feature = "test"))]
pub use self::fixtures::{
    close_initial_services, emby_unified_state, install_test_emby, live_owner_channel,
    make_audio_items, make_audio_only_remote_app_stub_with_cmd_rx, make_items,
    make_local_daemon_app_stub, make_local_daemon_app_stub_with_cmd_rx,
    make_local_daemon_app_stub_with_cmd_rx_and_compat, make_remote_app_stub,
    make_remote_app_stub_at_index, make_remote_app_stub_with_cmd_rx, remote_stub_config,
    seed_queue_row_menu_rows_for_test, seed_queue_shuffle_owner_for_test,
};
/// One seeded episode row of the GUI's Podcasts helpers: `(episode id, title,
/// description, publish instant in unix seconds, duration seconds)`.
#[cfg(any(test, feature = "test"))]
pub type SeededAbsEpisode<'a> = (&'a str, &'a str, Option<&'a str>, Option<u64>, Option<f64>);

/// One seeded book row of the GUI's Audiobooks catalog helper: `(id, title,
/// author display, author sort key, duration seconds, cover path)`.
#[cfg(any(test, feature = "test"))]
pub type SeededAbsBook<'a> = (
    &'a str,
    &'a str,
    Option<&'a str>,
    &'a str,
    f64,
    Option<&'a str>,
);

#[cfg(test)]
pub(crate) use self::named_queues::{HOME_POSITION, PEER_POSITION};
#[cfg(any(test, feature = "test"))]
pub use self::named_queues::{
    current, gui_app_with_named_queues, gui_app_with_peer, inject_local_events, named_queue_list,
    op_answer, resting, resting_snapshot,
};

pub use self::built::handoff_app;
#[cfg(any(test, feature = "test"))]
pub use self::built::make_app_stub;

/// The GUI `sections` bench's bench-scale seeds (design D12 of
/// `gui-change-driven-sections`), in their own child module.
#[cfg(any(test, feature = "test"))]
mod gui_scale;

/// The capabilities a GUI Client presents (design D6a of
/// `decouple-client-core`). It is the headless stand-in for the set
/// `attach_owner` records at a real GUI attach, so a GUI test gives its stub
/// this value instead of setting capability flags by hand.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn gui_client_capabilities() -> crate::ClientCapabilities {
    crate::ClientCapabilities::empty()
        .with(crate::ClientCapability::ShownQueueEntries)
        .with(crate::ClientCapability::TransportChipKeys)
}

fn apply_app_stub_defaults(app: &mut App) {
    // `build` doubles the configured image cache size; the stub keeps its
    // original (undoubled) budget so eviction behaviour is unchanged, under
    // the `test` support build it was set from.
    #[cfg(any(test, feature = "test"))]
    app.images.set_cache_capacity_for_test(50);
    // The stub stands in for a TUI Client without an Owner attach (design
    // D6a of `decouple-client-core`): it holds the capabilities the TUI
    // presents, which `App::build` leaves empty until a Client sets them.
    app.set_client_capabilities(
        crate::ClientCapabilities::empty()
            .with(crate::ClientCapability::TerminalPanelRules)
            .with(crate::ClientCapability::VisualSlotControls),
    );
    // Ignore any on-disk feed entry state; a stub starts empty.
    app.feed_entry_state = mbv_feed::FeedEntryStore::default();
    // Default to a focused window that reported focus in the past, so
    // existing mouse tests dispatch without arming focus explicitly.
    app.refocus_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .unwrap(),
    );
}

/// The `App` a Client's test drives, with `n` projected queue slots so a
/// list surface has content. The slots are Emby movie stubs in canonical
/// order, cursor at the first.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_queue(n: usize) -> App {
    let mut app = make_app_stub();
    app.local_view = queue_view(n);
    // The stub player is idle, so the Queue playback band collapses and the
    // list keeps the whole window (task 3.6).
    app.player.update_status(|status| status.active = false);
    app
}

/// The `App` a GUI Client's test drives with `n` queue slots, holding the
/// [`gui_client_capabilities`] a real GUI attach records (design D6a of
/// `decouple-client-core`), over a player whose command channel stays open and
/// the sender of its Owner-event channel. The stub capabilities are the TUI's
/// preset, which gates every GUI-only control, and the plain stub's player
/// channel is already dropped, so a test of a control takes this App, the
/// receiver of the Owner commands it sends, and the events it injects. The
/// Owner's two-queue list is adopted and its current queue shown, so the header
/// controls act on a named entry (design D7 of `gui-named-queues`).
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_for_gui_client(
    n: usize,
) -> (
    App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
    std::sync::mpsc::Sender<mbv_ctrl::player::PlayerEvent>,
) {
    let mut app = stub_app_with_queue(n);
    app.set_client_capabilities(gui_client_capabilities());
    let (remote, _player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.update_status(|status| status.volume_max = 100);
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    let (tx, rx) = std::sync::mpsc::channel();
    app.player_rx = rx;
    let current = mbv_queue::NamedQueueId::from_raw(1);
    app.adopt_named_queue_list(
        vec![
            mbv_ctrl::NamedQueueEntry {
                id: current,
                name: "Default".to_string(),
                len: n,
            },
            mbv_ctrl::NamedQueueEntry {
                id: mbv_queue::NamedQueueId::from_raw(2),
                name: "Kids".to_string(),
                len: 1,
            },
        ],
        current,
    );
    app.show_local_current_entry();
    (app, cmd_rx, tx)
}

/// The `App` a GUI Client's test drives with `n` queue slots over a player
/// whose command channel records what the actions send (add-tauri-gui-client
/// row 6.5): a drag's release or a key move lands one slot move the test
/// reads. The stub owner advertises no `answered-queue-ops`, so a queue edit
/// sends without waiting for an answer.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_queue_and_cmd_rx(
    n: usize,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut app, cmd_rx) = app_with_cmd_rx();
    app.local_view = queue_view(n);
    app.player.update_status(|status| status.active = false);
    (app, cmd_rx)
}

/// The stub `App` over a player whose command channel records what an action
/// sends (add-tauri-gui-client row 6.5): the stub owner advertises no
/// `answered-queue-ops`, so a queue edit sends without waiting for an answer.
/// The construction's own preference sync lands on the channel before the
/// caller starts, so it is drained here and a test sees only what its actions
/// send.
#[cfg(any(test, feature = "test"))]
fn app_with_cmd_rx() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    let mut compatibility = mbv_ctrl::CtrlCompatibility::current();
    compatibility.supports_answered_queue_ops = false;
    remote.set_ctrl_compatibility_for_test(compatibility);
    let player = mbv_player::PlayerProxy::from_remote(remote, false);
    let mut app = built::built_app(player, player_rx);
    apply_app_stub_defaults(&mut app);
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

/// The projected queue view the queue stubs share: `n` Emby movie slots in
/// canonical order, numbered from 1.
#[cfg(any(test, feature = "test"))]
fn queue_view(n: usize) -> QueueView {
    let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = (0..n)
        .map(|i| mbv_ctrl::UnifiedQueueSlot {
            slot_id: u64::try_from(i).unwrap_or(u64::MAX) + 1,
            item: mbv_queue::QueueItem::Emby(Box::new(mbv_emby_model::test_support::make_item(
                &format!("Item {i}"),
                "Movie",
            ))),
        })
        .collect();
    let state = mbv_ctrl::UnifiedQueueStateData {
        queue: None,
        start_slot: None,
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: None,
        slots,
        revision: 1,
        source: mbv_queue::QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    QueueView::from_snapshot(&state)
}

/// The `App` a Client's test drives with `n` projected queue slots and the
/// first one playing, so the Queue playback band shows its transport
/// (`extract-client-core` row 6.5).
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_playing_queue(n: usize) -> App {
    let app = stub_app_with_queue(n);
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.queue_len = n;
    });
    app
}

/// The `App` a Client's test drives with the queue's slots built from
/// `items` in canonical order and the first one playing, so a frame test can
/// name watched, partial-progress, and active rows from the items' own
/// stored facts (add-tauri-gui-client row 6.2). The playing slot starts
/// fresh: the status carries the first item's runtime and no position, so a
/// queued item's stored progress is the partial-progress row's own fact.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_playing_items(items: Vec<mbv_queue::QueueItem>) -> App {
    let app = stub_app_with_items(items);
    let runtime = app
        .playback_queue()
        .item_at(0)
        .map_or(0, mbv_queue::QueueItem::runtime_ticks);
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.queue_len = app.playback_queue().slots().len();
        status.runtime_ticks = runtime;
    });
    app
}

/// The `App` a Client's test drives with the queue's slots built from
/// `items` in canonical order, cursor at the first, and an idle player.
#[cfg(any(test, feature = "test"))]
#[must_use]
fn stub_app_with_items(items: Vec<mbv_queue::QueueItem>) -> App {
    let mut app = make_app_stub();
    let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = items
        .into_iter()
        .enumerate()
        .map(|(i, item)| mbv_ctrl::UnifiedQueueSlot {
            slot_id: u64::try_from(i).unwrap_or(u64::MAX) + 1,
            item,
        })
        .collect();
    let state = mbv_ctrl::UnifiedQueueStateData {
        queue: None,
        start_slot: None,
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: None,
        slots,
        revision: 1,
        source: mbv_queue::QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    app.local_view = QueueView::from_snapshot(&state);
    app
}

/// The `App` a Client's test drives with `n` projected queue slots and the
/// queue marked unsaved, so the Queue footer carries its dirty pill
/// (`extract-client-core` row 6.4).
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_unsaved_queue(n: usize) -> App {
    let mut app = stub_app_with_queue(n);
    app.queue_dirty = true;
    app
}

/// The `App` a Client's test drives with prefix mode armed and `message`
/// flashed as a live Neutral toast, so the Status bar carries both
/// (`extract-client-core` row 6.5).
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_prefix_and_toast(message: &str) -> App {
    use mbv_ui_model::toast::ToastSeverity;

    let mut app = make_app_stub();
    app.prefix_armed = true;
    app.flash(message.to_string(), ToastSeverity::Neutral);
    app
}

/// The queue a mutation addresses on `link`: the link's own queue target, as
/// the dispatch arms and the queue-op seams build it.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn queue_target_for_test(
    link: mbv_ui_model::playback::QueueLink,
) -> crate::app::state::playback::QueueTarget {
    crate::app::state::playback::QueueTarget::current(link)
}

/// The resolver's normalized chord for a pressed key, as the TUI's neutral
/// input edge builds it before the router resolves it against the policy.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn key_chord_for_test(
    code: mbv_ui_msg::Key,
    mods: mbv_ui_msg::KeyModifiers,
) -> crate::app::input::resolver::KeyChord {
    crate::app::input::resolver::KeyChord::new(code, mods)
}

/// The queue card's cache key for the playing queue's item at `index`: the
/// key the card projection fetches and paints that row's artwork under.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn queue_card_cache_key_for_test(app: &App, index: usize) -> Option<String> {
    let item = app.playback_queue().emby_item_at(index)?;
    Some(crate::app::state::projection::card::artwork::card_cache_key(item))
}

/// The queue card's cache key for an item id and type, as the slotless
/// session path builds it.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn card_cache_key_for_id_for_test(item_id: &str, item_type: Option<&str>) -> String {
    crate::app::state::projection::card::artwork::card_cache_key_for_id(item_id, item_type)
}

/// Hold a populated Emby movie library on the `App`: the Emby state a key or
/// action that leaked across the Service seam would mutate.
#[cfg(any(test, feature = "test"))]
pub fn push_movies_library_for_test(app: &mut App) {
    let mut library = mbv_emby_model::test_support::make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.push_library_for_test(mbv_ui_model::library_tab::LibraryTab {
        nav_stack: vec![mbv_ui_model::browse::BrowseLevel {
            rows: mbv_ui_model::browse::ServerRows::new(1),
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![mbv_emby_model::test_support::make_item("Item 0", "Movie")],
            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..mbv_ui_model::library_tab::LibraryTab::new(library)
    });
}

/// The `App` a GUI Client's test drives with the cast receivers replaced by
/// `receivers` (add-tauri-gui-client row 6.5's paint/press regression): the
/// target list the last emitted frame painted can then differ from the list
/// a press would read, so a press must address the painted choice.
#[cfg(any(test, feature = "test"))]
pub fn set_cast_receivers_for_test(app: &mut App, receivers: Vec<(&str, &str)>) {
    app.cast_receivers = receivers
        .into_iter()
        .map(|(id, name)| mbv_cast::discovery::CastReceiver {
            id: id.to_string(),
            friendly_name: name.to_string(),
            host: "192.168.0.5".to_string(),
            port: 8009,
        })
        .collect();
    app.rebuild_panel_targets();
}

/// Replace the local player's subtitle track list (add-tauri-gui-client row
/// 6.5's paint/press regression): the popover's painted rows then name a
/// list that changed before the press.
#[cfg(any(test, feature = "test"))]
pub fn set_subtitle_tracks_for_test(app: &mut App, tracks: Vec<(i64, String, bool)>) {
    app.player
        .update_status(|status| status.sub_tracks = tracks);
}

/// The `App` a Client's test drives with a pending settings save armed: the
/// config carries `consume_audio` and the save's deadline lies far in the
/// future, so only an exit boundary's flush can persist it — the periodic
/// maintenance the Client ticks run must not win the race the test observes.
///
/// # Panics
///
/// Panics while arming the save when the configuration lock is poisoned: a
/// writer panicked holding it, and the cached configuration cannot be
/// trusted.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_pending_settings_save() -> App {
    let mut app = make_app_stub();
    app.config.lock().unwrap().consume_audio = true;
    app.settings_save_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3600));
    app
}

/// The `App` a Client's test drives with `quit` rebound to `chord` in the
/// Session section of the compiled keybinds: the one router resolves the
/// action's configured chords, so the rebound chord routes to quit in the
/// GUI exactly as a user's `[keys]` override would (add-tauri-gui-client
/// row 6.2: the GUI-only context captures the default `q` for the fold).
///
/// # Panics
///
/// Panics while parsing the chord when `chord` is not a chord grammar
/// string.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_rebound_quit(chord: &str) -> App {
    use mbv_keybinds::{Chord, KeySection, Keybinds, SectionBindings};

    let app = stub_app_with_queue(2);
    app.config.lock().unwrap().keybinds = Keybinds {
        prefix: None,
        sections: vec![(
            KeySection::Session,
            SectionBindings {
                router: vec![("quit", Chord::parse(chord).expect("test chord must parse"))],
                prefix: vec![],
            },
        )],
    };
    app
}

/// The `App` a GUI Client's test drives with a cast receiver the Owner
/// offers as a playback target and a ten-minute item playing
/// (add-tauri-gui-client row 6.4): the `panel_targets` merge holds the
/// receiver, the player's Owner advertises the `playback-output`
/// capability the cast row's presence is gated on, and the command channel
/// records what a chooser selection or a transport request sends to the
/// Owner. The receiver's id is `receiver-1`.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_cast_target() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    let mut compatibility = mbv_ctrl::CtrlCompatibility::current();
    compatibility.supports_playback_output = true;
    remote.set_ctrl_compatibility_for_test(compatibility);
    remote.update_status(|status| {
        status.active = true;
        status.runtime_ticks = 10 * 60 * mbv_emby_model::TICKS_PER_SECOND;
    });
    let player = mbv_player::PlayerProxy::from_remote(remote, false);
    let mut app = built::built_app(player, player_rx);
    apply_app_stub_defaults(&mut app);
    app.cast_receivers = vec![mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }];
    app.rebuild_panel_targets();
    // The construction's preference sync lands on the channel before the
    // test starts; drain it so the test sees only commands its actions send.
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

/// The `App` a GUI Client's test drives with a failed Emby setup form whose
/// password field still holds the typed secret (`add-gui-settings-screen` row
/// 4.3): the Client opens the Settings screen itself, and the frame's Services
/// content must carry the error and the server URL and never the password.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_failed_emby_setup_form() -> App {
    use crate::app::dispatch::session::services_settings::EmbySetupForm;
    use mbv_core::service_runtime::ServiceState;

    let mut app = make_app_stub();
    app.setup.emby_setup_form = Some(EmbySetupForm {
        fields: [
            "https://emby.example.test".to_string(),
            "viewer".to_string(),
            "hunter2".to_string(),
        ],
        focus: 2,
        busy: false,
        error: "Invalid username or password".to_string(),
        generation: None,
        previous_state: ServiceState::NotConfigured,
    });
    app
}

/// The `App` a GUI Client's test drives with an active local player
/// carrying one audio track (id 1, chosen) and one forced subtitle track
/// (id 2; id 0, Off, is chosen), so a chip press opens its popover and a
/// choice records on the command channel (add-tauri-gui-client row 6.4):
/// the same shape `stub_app_with_track_popover` arms, over a remote stub
/// that exposes what a track switch sends the Owner.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_track_choices() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.update_status(|status| {
        status.active = true;
        status.audio_tracks = vec![(1, "Stereo".to_string())];
        status.audio_id = 1;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });
    let player = mbv_player::PlayerProxy::from_remote(remote, false);
    let mut app = built::built_app(player, player_rx);
    apply_app_stub_defaults(&mut app);
    // The construction's preference sync lands on the channel before the
    // test starts; drain it so the test sees only commands its actions send.
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

/// The `App` a GUI Client's test drives with the Emby client an attach
/// brings (`owner_link`'s `cached_emby_client`): the runtime is Ready and
/// holds `client`, so the shared startup step starts no Emby startup worker
/// (add-gui-library-panel design D11). The test builds the client over an
/// isolated mock transport, so no request leaves the process.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_cached_emby_client(client: mbv_emby::EmbyClient) -> App {
    let mut app = make_app_stub();
    app.emby_runtime =
        crate::app::state::service_runtime::EmbyRuntime::ready(Arc::new(Mutex::new(client)));
    app
}

/// The library id the seeded Music library tab holds.
#[cfg(any(test, feature = "test"))]
pub const GUI_MUSIC_LIBRARY_ID: &str = "lib-music";

/// The library id the seeded Shows library tab holds.
#[cfg(any(test, feature = "test"))]
pub const GUI_TV_LIBRARY_ID: &str = "lib-tv";

/// A music group level: the library root's children, its cursor on the first.
#[cfg(any(test, feature = "test"))]
fn music_group_level(groups: Vec<mbv_emby_model::EmbyItem>) -> mbv_ui_model::browse::BrowseLevel {
    use mbv_ui_model::browse::{BrowseResting, ServerRows};

    mbv_ui_model::browse::BrowseLevel {
        parent_id: GUI_MUSIC_LIBRARY_ID.to_string(),
        title: "Music".to_string(),
        items: groups,
        rows: ServerRows::new(0),
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".to_string(),
        sort_order: "Ascending".to_string(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

/// A group's album level: the group's children, its cursor on the first.
#[cfg(any(test, feature = "test"))]
fn music_album_level(albums: Vec<mbv_emby_model::EmbyItem>) -> mbv_ui_model::browse::BrowseLevel {
    let mut level = music_group_level(albums);
    level.parent_id = "grp-hiphop".to_string();
    level.title = "Hip-Hop".to_string();
    level
}

/// A group row of the music group level.
#[cfg(any(test, feature = "test"))]
fn group_item(id: &str, name: &str) -> mbv_emby_model::EmbyItem {
    let mut item = mbv_emby_model::test_support::make_item(name, "MusicArtist");
    item.id = id.to_string();
    item.is_folder = true;
    item
}

/// An album row of a group's album level: a folder with no playable media of
/// its own, as Emby returns a `MusicAlbum`.
#[cfg(any(test, feature = "test"))]
fn album_item(id: &str, name: &str) -> mbv_emby_model::EmbyItem {
    let mut item = mbv_emby_model::test_support::make_item(name, "MusicAlbum");
    item.id = id.to_string();
    item.is_folder = true;
    item.media_type.clear();
    item
}

#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_completed_emby_startup_and_contents(
    views: Vec<mbv_emby_model::EmbyItem>,
    continue_items: Vec<mbv_emby_model::EmbyItem>,
) -> App {
    let mut app = make_app_stub();
    let generation = app.emby_runtime.generation();
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = tx.send(crate::app::dispatch::session::service_startup::Completion {
        generation,
        result: Ok(crate::app::dispatch::session::service_startup::Startup {
            client: mbv_emby::EmbyClient::new(crate::app::config::Config::default()),
            bootstrap: mbv_emby::EmbyBootstrap {
                continue_items,
                views,
            },
            setup: mbv_config::EmbySetup::default(),
        }),
    });
    app.setup.emby_startup_rx =
        Some(crate::app::dispatch::session::service_startup::StartupReceiver { generation, rx });
    app
}

/// The `App` a GUI Client's test drives with a successful Emby startup
/// completion already queued for the startup drain, over `views` as the
/// bootstrap's library catalog. The worker's thread never runs: the test
/// hands the drain the one completion a worker would send (add-gui-library-panel
/// design D11).
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_completed_emby_startup(views: Vec<mbv_emby_model::EmbyItem>) -> App {
    stub_app_with_completed_emby_startup_and_contents(views, Vec::new())
}

/// The Client tests' test-only reads and seeds of an `App`'s state
/// (`app_stub::test_reads`).
#[cfg(any(test, feature = "test"))]
pub(in crate::app) mod test_reads;

/// The `App` a GUI Client's Owner-exit test drives (`daemon-disconnect-`
/// `handling` "Clients exit cleanly on an announced shutdown"): the stub
/// queue's App attached at the local endpoint — the route the announce's
/// exit path checks — with the returned sender feeding its player-event
/// channel, because the channel itself is behind `pub(in crate::app)` and
/// a Client's cross-crate test injects through this seam. The constructor
/// sends no player event, so a test's sends are the first ones.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_local_daemon_events(
    n: usize,
) -> (App, std::sync::mpsc::Sender<mbv_ctrl::player::PlayerEvent>) {
    let mut app = stub_app_with_queue(n);
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    let (tx, rx) = std::sync::mpsc::channel();
    app.player_rx = rx;
    (app, tx)
}
