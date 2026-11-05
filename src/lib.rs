//! The `mbv` library: the shell `App`, the Client core tick that every Client
//! drives, the central keyboard router, and the Client-side Owner link
//! (design D2 and D7 of `add-gui-front-end`).
//!
//! The `mbv` binary is a thin `main` over [`run`], and the GUI Client
//! (`mbv-gui`) shares the rest of this surface: the core tick, the router
//! entry, and the Owner find, start, and attach steps. The library still
//! carries the TUI Client's dependencies — `ratatui`, `crossterm`, and
//! `TuiRealm` — so any Client that links it links them; it never carries a
//! window stack.

mod app;
mod config;
mod launch;
mod local_daemon;
mod owner_link;
mod owner_restart;
mod pin;
mod single_instance;
mod startup;

pub use self::app::{
    App, AppCommand, CLIENT_IDLE_CADENCE, ClientCore, ClientLifecycle, ClientTickHost, ClientWaker,
    Command, GuiInput, GuiSurfaceLeaf, HomeContent, HomeEventDrain, NavigationIntent, OverlayFocus,
    PanelIntent, PlaybackIntent, PlayerEventDrain, QueueSurfaceProjection, RemoteControlTarget,
    RouterOutcome, RouterOverlayState, RouterPlaybackState, RouterSnapshot, ShellCommand,
    StartupWorkerDrain, TickOutcome, VisualIntent, WorkspaceRelayout,
    resolve_router_outcome_with_focused,
};
pub use self::config::{Config, load_config};
pub use self::owner_link::{
    OwnerStartOptions, OwnerState, attach_owner, declare_gui_client, exclusive_owner_refusal_text,
    find_owner, refusal_text, start_owner,
};
pub use self::startup::run;

/// The `App` stub construction a Client's own tests build their `App` from
/// (`add-gui-front-end` design D11). Compiled only under the root package's
/// `test` feature, which no shipped build enables.
#[cfg(feature = "test")]
pub use self::app::stub_app_with_queue;
