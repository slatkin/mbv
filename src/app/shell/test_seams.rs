//! Shell-side test seams (`decouple-client-core` design D3: the seams that name
//! a toolkit live in `src/app/shell/`, and the toolkit-free ones stay in
//! `src/app/test_seams.rs`).
//!
//! The seams below belong to the TUI Client's runtime alone: the terminal
//! listener (`shell/input_port.rs`) owns the session lookup and the one connect
//! path it drives. A core module never reads them, so a Client that presents no
//! terminal, shell, or Hero link never carries the seam. The shell reads them
//! exactly as the core did, so a test injects the same way and behaviour is
//! unchanged.

use std::sync::{Mutex, mpsc};

pub(in crate::app) type DirectConnectFn = fn(
    &mbv_remote_player::DaemonEndpoint,
) -> Result<
    (
        mbv_remote_player::RemotePlayer,
        mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
    ),
    mbv_remote_player::RemotePlayerError,
>;

pub(in crate::app) static DIRECT_CONNECT_OVERRIDE: Mutex<Option<DirectConnectFn>> =
    Mutex::new(None);
pub(in crate::app) static DIRECT_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());

// Separate from DIRECT_CONNECT_OVERRIDE above (Sessions-panel "Direct
// Remote" upgrade, keyed off a discovered SessionInfo): this is issue
// #222's lazy daemon-route connect primitive, targeting a statically
// configured DaemonEndpoint with no session discovery. Kept as its own
// override/lock pair so the two connect paths -- and the App state they
// drive (`direct_remote_label` vs. `active_route`) -- stay independently
// testable and are never conflated, per #223's explicit rule.
pub(in crate::app) type DaemonRouteConnectFn = fn(
    &mbv_remote_player::DaemonEndpoint,
) -> (
    mbv_remote_player::RemotePlayer,
    mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
);
pub(in crate::app) static DAEMON_ROUTE_CONNECT_OVERRIDE: Mutex<Option<DaemonRouteConnectFn>> =
    Mutex::new(None);
pub(in crate::app) static DAEMON_ROUTE_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());

// Test seam for live-session-list lookups, mirroring
// DAEMON_ROUTE_CONNECT_OVERRIDE/_TEST_LOCK above: lets tests inject a fake
// session list without a real network call. Shared by
// `try_auto_reconnect`'s `DirectSession` lookup (#236) and the F2
// "Library Routes" device picker (`enter_device_stage`, #256).
pub(in crate::app) type SessionsLoadFn =
    fn(&mbv_emby::EmbyClient) -> Result<Vec<mbv_emby::SessionInfo>, mbv_emby::EmbyError>;
pub(in crate::app) static SESSIONS_LOAD_OVERRIDE: Mutex<Option<SessionsLoadFn>> = Mutex::new(None);
