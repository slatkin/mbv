//! Public-API tests of the GUI Client's shared Service startup step
//! (add-gui-library-panel design D11): the core thread runs the same
//! `App::start_client_services` and launch Home load the TUI Client runs, and
//! it keeps the playback-target discovery the deleted minimal Emby client
//! fill used to start. The Emby responses come from the in-memory mock
//! transport, so no request leaves the process and no Owner runs (design D13).

use std::sync::mpsc;

use mbv::{
    App, ClientTickHost, Config, stub_app_with_cached_emby_client,
    stub_app_with_completed_emby_startup,
};
use mbv_emby::EmbyClient;
use mbv_emby_model::EmbyItem;
use mbv_gui::core_thread::{CoreInput, CoreThread, FrameSink, QuitSink};
use mbv_net::mock_http::MockHttp;

/// A URL that never leaves the process: the mock transport ignores it, but it
/// must remain an IP literal so the default resolver never attempts DNS.
const TEST_URL: &str = "http://127.0.0.1:1";

/// The Emby `VirtualFolders` response that carries one Music library. Every
/// request the startup makes answers with it: the views fetch reads the
/// folder, and every other response path tolerates a bare array.
const MUSIC_VIEWS: &str = r#"[{"ItemId":"lib-music","Name":"Music","CollectionType":"music"}]"#;

/// The one Music library the catalog must hold.
fn music_view() -> EmbyItem {
    EmbyItem::folder("lib-music".into(), "Music".into(), "music".into())
}

/// A core thread over `app`, as the window's `main` builds after the attach.
fn core_over(app: App) -> CoreThread {
    let (_tx, rx) = mpsc::channel::<CoreInput>();
    let sink: FrameSink = Box::new(|_| {});
    let quit: QuitSink = Box::new(|| {});
    CoreThread::new(app, rx, sink, quit)
}

/// Whether the catalog holds the Music library tab.
fn has_music_tab(core: &mut CoreThread) -> bool {
    core.app_mut()
        .libs_for_test()
        .iter()
        .any(|tab| tab.library.name == "Music")
}

/// The regression guard for the discovery the deleted client fill ran: both
/// the `/Sessions` load and the cast discovery start.
fn assert_discovery_started(core: &mut CoreThread) {
    assert!(
        core.app_mut().sessions_load_started_for_test(),
        "the playback-target discovery must start the sessions load"
    );
    assert_eq!(
        core.app_mut().cast_discovery_starts_for_test(),
        1,
        "the playback-target discovery must start one cast discovery"
    );
}

/// A launch whose attach brings the cached Emby client runs no Emby startup
/// worker, so the launch Home load is what builds the catalog: the mocked
/// views response lands the Music tab in `App::libs`.
#[test]
fn a_cached_emby_client_landing_finds_the_music_tab_and_starts_the_discovery() {
    let http = MockHttp::new();
    // The views fetch and its siblings, in any interleaving with the
    // background loads: every response carries the same one-library catalog.
    http.respond_repeated(200, MUSIC_VIEWS, 8);
    let client = EmbyClient::new(Config {
        server_url: TEST_URL.into(),
        ..Config::default()
    })
    .with_test_agent(http.agent());
    let mut core = core_over(stub_app_with_cached_emby_client(client));

    assert!(
        has_music_tab(&mut core),
        "the cached launch lands the Music tab"
    );
    assert_discovery_started(&mut core);
}

/// An attach without a client starts one Emby startup worker. Its mocked
/// completion rebuilds the catalog, and the drain starts the playback-target
/// discovery the full startup itself never starts.
#[test]
fn a_mocked_emby_startup_completion_lands_the_catalog_and_starts_the_discovery() {
    let mut core = core_over(stub_app_with_completed_emby_startup(vec![music_view()]));

    assert!(
        core.drain_startup_workers(),
        "the drain applied the startup"
    );

    assert!(has_music_tab(&mut core), "the startup lands the Music tab");
    assert_discovery_started(&mut core);
}
