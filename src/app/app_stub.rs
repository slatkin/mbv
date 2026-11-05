//! A self-contained `App` stub for the Clients' own tests
//! (`add-gui-front-end` design D11): the ordinary `App::build` construction
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
    // Default to "focused, past grace window" so existing mouse tests dispatch
    // without arming focus explicitly. The refocus guard itself is tested
    // directly in input_music_track_focus_tests.
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
/// (`add-gui-front-end` row 6.5).
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

/// The ordinary `App::build` construction over stub channels: no live
/// Owner, no live Service, and the stub player whose command channel is
/// already dropped.
#[must_use]
pub fn make_built_app() -> App {
    use mbv_player::PlayerProxy;

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
