//! The GUI Client's minimal Emby fill (add-tauri-gui-client design D9's
//! ruled exception): `start_emby_client_fill` consumes the armed startup
//! request into an authentication-only worker — no library bootstrap —
//! `apply_emby_client_fill` installs the client and runs the discovery the
//! fill feeds, and the same reject the full startup's failure arm runs
//! classifies a failed authentication. All App construction flows through
//! `crate::app::tests::helpers`'s `make_app_stub`: no live Emby, no live
//! Owner.

use super::make_app_stub;
use crate::app::dispatch::session::service_startup::ClientFillCompletion;
use crate::config::Config;
use mbv_emby::EmbyClient;

/// A landed fill completion installs the authenticated client and starts
/// the session and cast discovery the fill feeds (design D9's ruled
/// exception): no bootstrap, no websocket, no capability registration.
#[test]
fn client_fill_installs_client_and_starts_discovery() {
    let mut app = make_app_stub();
    let generation = app.emby_runtime.generation();

    app.apply_emby_client_fill(ClientFillCompletion {
        generation,
        result: Ok(EmbyClient::new(Config::default())),
    });

    assert!(app.emby_client().is_some());
    assert!(
        app.sessions_loading,
        "the fill must spawn the sessions load"
    );
}

/// A stale fill completion applies nothing: the runtime accepts only its
/// own generation, as every Emby completion does.
#[test]
fn stale_client_fill_completion_is_ignored() {
    let mut app = make_app_stub();

    app.apply_emby_client_fill(ClientFillCompletion {
        generation: app.emby_runtime.generation().next(),
        result: Ok(EmbyClient::new(Config::default())),
    });

    assert!(app.emby_client().is_none());
}

/// `start_emby_client_fill` consumes the armed startup request into the
/// fill receiver the tick drains.
#[test]
fn start_client_fill_consumes_the_armed_request() {
    let mut app = make_app_stub();
    app.setup.emby_startup_request = Some(crate::app::state::service_setup::StartupRequest {
        config: Config::default(),
        generation: app.emby_runtime.generation(),
    });

    app.start_emby_client_fill();

    assert!(app.setup.emby_startup_request.is_none());
    assert!(app.setup.emby_client_fill_rx.is_some());
}

/// No armed request means no fill: an unconfigured Emby is a no-op, as is a
/// second call while the worker is in flight.
#[test]
fn start_client_fill_without_request_is_noop() {
    let mut app = make_app_stub();

    app.start_emby_client_fill();

    assert!(app.setup.emby_client_fill_rx.is_none());
}

/// A client that arrived with the attach (`owner_link`'s
/// `cached_emby_client`) needs no worker: the fill runs the session and
/// cast discovery it feeds directly, and consumes nothing.
#[test]
fn start_client_fill_with_client_starts_discovery() {
    let mut app = make_app_stub();
    app.emby_runtime.client = Some(std::sync::Arc::new(std::sync::Mutex::new(EmbyClient::new(
        Config::default(),
    ))));

    app.start_emby_client_fill();

    assert!(
        app.sessions_loading,
        "the fill must spawn the sessions load"
    );
    assert!(app.setup.emby_client_fill_rx.is_none());
}
