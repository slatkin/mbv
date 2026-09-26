use std::sync::{mpsc, Mutex};
use std::time::Duration;

pub(in crate::app) type DirectConnectFn = fn(
    &mbv_core::remote_player::DaemonEndpoint,
) -> Result<
    (
        mbv_core::remote_player::RemotePlayer,
        mpsc::Receiver<mbv_core::player::PlayerEvent>,
    ),
    String,
>;

pub(in crate::app) static DIRECT_CONNECT_OVERRIDE: Mutex<Option<DirectConnectFn>> =
    Mutex::new(None);

/// Test seam for local-player preparation. Production construction is still
/// the ordinary `Player::new` path; tests can inject a construction failure
/// without creating an mpv handle.
pub(in crate::app) type LocalPlayerPrepareFn = fn() -> Result<(), String>;
pub(in crate::app) static LOCAL_PLAYER_PREPARE_OVERRIDE: Mutex<Option<LocalPlayerPrepareFn>> =
    Mutex::new(None);

// Separate from DIRECT_CONNECT_OVERRIDE above (Sessions-panel "Direct
// Remote" upgrade, keyed off a discovered SessionInfo): this is issue
// #222's lazy daemon-route connect primitive, targeting a statically
// configured DaemonEndpoint with no session discovery. Kept as its own
// override/lock pair so the two connect paths -- and the App state they
// eventually drive (`connected_session_id`/`direct_remote_label` vs. a
// future #223 `active_route`) -- stay independently testable and are
// never conflated, per #223's explicit "must not be conflated" rule.
pub(in crate::app) static DAEMON_ROUTE_CONNECT_OVERRIDE: Mutex<Option<DirectConnectFn>> =
    Mutex::new(None);
pub(in crate::app) static DAEMON_ROUTE_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());

// Test seam for live-session-list lookups, mirroring
// DAEMON_ROUTE_CONNECT_OVERRIDE/_TEST_LOCK above: lets tests inject a fake
// session list without a real network call. Shared by
// `try_auto_reconnect`'s `DirectSession` lookup (#236) and the F2
// "Library Routes" device picker (`enter_device_stage`, #256).
pub(in crate::app) type SessionsLoadFn =
    fn(&mbv_core::api::EmbyClient) -> Result<Vec<mbv_core::api::SessionInfo>, String>;
pub(in crate::app) static SESSIONS_LOAD_OVERRIDE: Mutex<Option<SessionsLoadFn>> = Mutex::new(None);

// Test seam for `App::connect_cast_receiver`'s resolve-and-connect step
// (7.3/7.5), mirroring the overrides above: lets tests substitute a fake
// worker instead of a real mDNS browse + `CastClient::connect`, without
// making the reattach/attach-on-selection call sites themselves
// test-aware.
pub(in crate::app) type CastConnectFn =
    fn(&str, Duration) -> Result<mpsc::Sender<crate::app::state::types::cast::CastJob>, String>;
pub(in crate::app) static CAST_CONNECT_OVERRIDE: Mutex<Option<CastConnectFn>> = Mutex::new(None);
pub(in crate::app) static CAST_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());
