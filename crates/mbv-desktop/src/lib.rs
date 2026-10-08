//! Desktop surfaces for a Local process: MPRIS D-Bus server and system tray.
//!
//! The `mpris` media-player endpoint and the `tray` status icon live here for the local
//! Owner process and its Clients. The headless `mbvd` server daemon never has either:
//! desktop features belong to Local processes, which show the tray while Stay-alive is
//! enabled or the `Show systray icon` preference is on.

pub mod mpris;
pub mod tray;
