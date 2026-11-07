//! A self-contained `App` stub for the Clients' own tests
//! (`extract-client-core` design D11): the ordinary `App::build` construction
//! with stub-friendly defaults, no live Owner, and no live Service. The GUI
//! Client's headless tests build their `App` here, exactly as the TUI
//! Client's tests build theirs in `app::tests`.
//!
//! The module compiles under the root package's `test` feature as well as
//! under `cfg(test)`, so a dependent crate's dev-dependency can enable it;
//! the shipped builds never compile it.

use std::sync::{Arc, Mutex};

use super::{App, AppInit, QueueView};

/// Minimal `App` stub for logic-only tests: the ordinary `App::build`
/// construction ([`make_built_app`]) plus the stub-friendly defaults below.
#[must_use]
pub fn make_app_stub() -> App {
    let mut app = make_built_app();
    apply_app_stub_defaults(&mut app);
    app
}

fn apply_app_stub_defaults(app: &mut App) {
    // `build` doubles the configured image cache size; the stub keeps its
    // original (undoubled) budget so eviction behaviour is unchanged.
    app.images.set_cache_capacity_for_test(50);
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
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_queue(n: usize) -> App {
    let mut app = make_app_stub();
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
    // The stub player is idle, so the Queue playback band collapses and the
    // list keeps the whole window (task 3.6).
    app.player.update_status(|status| status.active = false);
    app
}

/// The `App` a Client's test drives with `n` projected queue slots and the
/// first one playing, so the Queue playback band shows its transport
/// (`extract-client-core` row 6.5).
#[cfg(feature = "test")]
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
#[cfg(feature = "test")]
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
#[cfg(feature = "test")]
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
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_unsaved_queue(n: usize) -> App {
    let mut app = stub_app_with_queue(n);
    app.queue_dirty = true;
    app
}

/// The `App` a Client's test drives with prefix mode armed and `message`
/// flashed as a live Neutral toast, so the Status bar carries both
/// (`extract-client-core` row 6.5).
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_prefix_and_toast(message: &str) -> App {
    use mbv_ui_model::toast::ToastSeverity;

    let mut app = make_app_stub();
    app.prefix_armed = true;
    app.flash(message.to_string(), ToastSeverity::Neutral);
    app
}

/// The `App` a Client's test drives with the queue built from `items` and
/// the row menu open over its first slot, so a frame test reads the open
/// menu's entries and their key hints (add-tauri-gui-client row 6.2). The
/// menu is the same one `RowContextMenu`'s open path builds.
///
/// # Panics
///
/// Panics while resolving the menu's row when `items` is empty: the stub
/// queue then has no first slot to open the menu over.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_open_row_menu(items: Vec<mbv_queue::QueueItem>) -> App {
    let mut app = stub_app_with_playing_items(items);
    let row = app
        .displayed_queue()
        .slot_id_at(0)
        .expect("the stub queue has a first slot");
    app.open_gui_row_menu(&[row]);
    app
}

/// The `App` a Client's test drives with an active player carrying one audio
/// and one subtitle track, and the track chip's popover open on `chip`, so a
/// frame test reads the popover's choices with their chosen marks
/// (add-tauri-gui-client row 6.2).
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_track_popover(chip: mbv_ui_msg::TrackChipKind) -> App {
    use mbv_ui_msg::PlaybackRequest;

    let mut app = stub_app_with_playing_queue(1);
    app.player.update_status(|status| {
        status.active = true;
        status.audio_tracks = vec![(1, "Stereo".to_string())];
        status.audio_id = 1;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });
    app.handle_playback_request(&PlaybackRequest::OpenTrackPopover(chip));
    app
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
#[cfg(feature = "test")]
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
#[cfg(feature = "test")]
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
#[cfg(feature = "test")]
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
    let mut app = built_app(player, player_rx);
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

/// The `App` a GUI Client's test drives with the Emby startup request
/// armed, as `initialize_service_startup` arms it for an attached session
/// whose Emby is configured without a client, so the test reads the
/// minimal client fill the core starts (add-tauri-gui-client design D9's
/// ruled exception). The request's worker resolves against the isolated
/// state dir, where no credential exists.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_armed_emby_fill() -> App {
    let mut app = make_app_stub();
    app.setup.emby_startup_request = Some(crate::app::state::service_setup::StartupRequest {
        config: crate::config::Config::default(),
        generation: app.emby_runtime.generation(),
    });
    app
}

/// The ordinary `App::build` construction over stub channels: no live
/// Owner, no live Service, and the stub player whose command channel is
/// already dropped.
#[must_use]
pub fn make_built_app() -> App {
    use mbv_player::PlayerProxy;

    let (remote, player_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);
    remote.update_status(|status| status.volume_max = 100);
    built_app(PlayerProxy::from_remote(remote, false), player_rx)
}

/// The `App::build` construction over `player` and its event channel, the
/// shared half of every stub that needs its own player wiring (a command
/// channel of its own, say).
fn built_app(
    player: mbv_player::PlayerProxy,
    player_rx: std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
) -> App {
    let (_, ws_rx) = std::sync::mpsc::channel();
    let (_, transport_rx) = std::sync::mpsc::channel();
    let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
    let channels = crate::app::state::runtime_channels::RuntimeChannels::new();

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
