//! The stub `App` constructions over `App::build`: no live Owner, no live
//! Service, and the player the caller wires.
//!
//! The construction half (`built_app`, `handoff_app`, `make_app_stub`)
//! compiles in every build, because a Client wrapper parks an `App` of this
//! shape while it holds the live one (the GUI's own keyboard router, its tab
//! step, and the Settings overlay step). The rest is the test and `test`-build
//! stubs.

use std::sync::{Arc, Mutex};

use super::super::{App, AppInit, QueueView};
use super::apply_app_stub_defaults;

/// Minimal `App` stub for logic-only tests: the ordinary `App::build`
/// construction ([`make_built_app`]) plus the stub-friendly defaults
/// (`apply_app_stub_defaults`).
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_app_stub() -> App {
    let mut app = make_built_app();
    apply_app_stub_defaults(&mut app);
    app
}

/// The ordinary `App::build` construction over stub channels: no live
/// Owner, no live Service, and the stub player whose command channel is
/// already dropped.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn make_built_app() -> App {
    make_built_app_ungated()
}

/// The build construction, ungated: the `test` stub and the parked `App` both
/// start here.
#[cfg(any(test, feature = "test"))]
fn make_built_app_ungated() -> App {
    let empty: Vec<mbv_emby_model::EmbyItem> = Vec::new();
    let (remote, player_rx) = mbv_remote_player::RemotePlayer::stub(empty, 0);
    remote.update_status(|status| status.volume_max = 100);
    built_app(
        mbv_player::PlayerProxy::from_remote(remote, false),
        player_rx,
    )
}

/// The parked `App`'s player: no Owner connection and no event, because the
/// parked value never outlives the one call that moved the live one out.
#[cfg(not(any(test, feature = "test")))]
fn make_built_app_ungated() -> App {
    let empty: Vec<mbv_emby_model::EmbyItem> = Vec::new();
    let (remote, player_rx) = mbv_remote_player::RemotePlayer::stub_for_parked_app(empty, 0);
    built_app(
        mbv_player::PlayerProxy::from_remote(remote, false),
        player_rx,
    )
}

/// An `App` that stands in for the one a wrapper moves out before it wraps it
/// (`GuiCore::new(std::mem::replace(..))`): the stub construction, which the
/// wrapper drops as soon as the real `App` comes back.
#[must_use]
pub fn handoff_app() -> App {
    let app = make_built_app_ungated();
    apply_handoff_defaults(app)
}

/// The parked `App`'s defaults: the focus and feed-entry defaults a stub gets,
/// through the same helper, so a parked and a stub `App` start the same.
fn apply_handoff_defaults(mut app: App) -> App {
    apply_app_stub_defaults(&mut app);
    app
}

/// The `App::build` construction over `player` and its event channel: the
/// shared half of every stub that needs its own player wiring (a command
/// channel of its own, say).
pub(in crate::app) fn built_app(
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
        indicator_style: mbv_ui_model::indicators::IndicatorStyle::default(),
        image_cache_size: 50,
        image_decode: mbv_images::ImageDecode::Pixels,
        visualizer_glyph: crate::config::DEFAULT_VISUALIZER_GLYPH.into(),
        card_image_tx,
        card_image_rx,
        channels,
        idle_feed: None,
    })
}
