use std::sync::{Mutex, mpsc};

pub type DirectConnectFn = fn(
    &mbv_remote_player::DaemonEndpoint,
) -> Result<
    (
        mbv_remote_player::RemotePlayer,
        mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
    ),
    mbv_remote_player::RemotePlayerError,
>;

pub static DIRECT_CONNECT_OVERRIDE: Mutex<Option<DirectConnectFn>> = Mutex::new(None);
pub static DIRECT_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());

// Separate from DIRECT_CONNECT_OVERRIDE above (Sessions-panel "Direct
// Remote" upgrade, keyed off a discovered SessionInfo): this is issue
// #222's lazy daemon-route connect primitive, targeting a statically
// configured DaemonEndpoint with no session discovery. Kept as its own
// override/lock pair so the two connect paths -- and the App state they
// drive (`direct_remote_label` vs. `active_route`) -- stay independently
// testable and are never conflated, per #223's explicit rule.
pub type DaemonRouteConnectFn = fn(
    &mbv_remote_player::DaemonEndpoint,
) -> (
    mbv_remote_player::RemotePlayer,
    mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
);
pub static DAEMON_ROUTE_CONNECT_OVERRIDE: Mutex<Option<DaemonRouteConnectFn>> = Mutex::new(None);
pub static DAEMON_ROUTE_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());

// Test seam for live-session-list lookups, mirroring
// DAEMON_ROUTE_CONNECT_OVERRIDE/_TEST_LOCK above: lets tests inject a fake
// session list without a real network call. Shared by
// `try_auto_reconnect`'s `DirectSession` lookup (#236) and the F2
// "Library Routes" device picker (`enter_device_stage`, #256).
pub type SessionsLoadFn =
    fn(&mbv_emby::EmbyClient) -> Result<Vec<mbv_emby::SessionInfo>, mbv_emby::EmbyError>;
pub static SESSIONS_LOAD_OVERRIDE: Mutex<Option<SessionsLoadFn>> = Mutex::new(None);

// Test seam for URL opens (the GUI Library `openUrl` command, the Hero's IMDb
// link): the command path reads it after the scheme gate, so a test observes
// the url the opener receives without a real `xdg-open`/`open` spawn.
pub type OpenUrlFn = fn(&str);
pub static OPEN_URL_OVERRIDE: Mutex<Option<OpenUrlFn>> = Mutex::new(None);
pub static OPEN_URL_TEST_LOCK: Mutex<()> = Mutex::new(());
