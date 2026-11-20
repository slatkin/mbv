//! Desktop surfaces for a Local process: MPRIS D-Bus server and system tray.
//!
//! The `mpris` media-player endpoint serves two publishers (design D1): the local
//! Owner process holds `org.mpris.MediaPlayer2.mbv` for its lifetime
//! (`mpris::start_owner`), and a Client that controls a remote Player owner holds
//! `org.mpris.MediaPlayer2.mbv.instance<pid>` only while the local Owner is not playing.
//! The `tray` status icon lives here for the local Owner process and its Clients. The
//! headless `mbvd` server daemon never has either: desktop features belong to Local
//! processes, which show the tray while Stay-alive is enabled or the `Show systray
//! icon` preference is on.

pub mod mpris;
pub mod tray;
