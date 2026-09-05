//! `Msg` and its request payloads (design D4).
//!
//! `Msg` carries only cross-authority requests; local state changes never
//! become a `Msg` (they mutate the component in `on`/`update` and return
//! `None`). Request payloads are placeholder scaffolds filled in as each
//! surface converts (see per-type TODOs).
//!
//! Task 8.3 split the per-family request/intent enums into submodules so
//! this file stays below the 800-line cap. Re-exports preserve the
//! `crate::app::components::msg::TypeName` import path used throughout the
//! codebase; nothing else had to change.

use tuirealm::event::KeyEvent as TuiKeyEvent;

mod hit_regions;
mod intents;
mod playback;
mod queue;
mod service;
mod shell;

pub use self::hit_regions::TvHit;
pub use self::intents::{
    AlbumCursorKind, AudiobookshelfBookIntent, AudiobookshelfBookMove, ConfirmIntent,
    ContextMenuIntent, DaemonLostIntent, FeedsManageIntent, PodcastEpisodeIntent,
    PodcastEpisodeTransition, RemoteReanchorIntent, SavePlaylistIntent, SettingsIntent,
};
pub use self::playback::PlaybackRequest;
pub use self::queue::{QueueColumnResize, QueueIntent, QueueMove, QueueRequest};
pub use self::service::ServiceRequest;
pub use self::shell::ShellRequest;

/// The single TuiRealm outbound type, grouping surface output enums (design
/// D4). `Application` requires `Msg: PartialEq`; convenience `Debug`/`Clone`
/// derives aid diagnostics and follow-on message cascades.
// TODO(migrate-tui-to-tuirealm): box the large request variant after migration churn settles.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Navigate(NavTarget),
    Playback(PlaybackRequest),
    Queue(QueueRequest),
    Service(ServiceRequest),
    Shell(ShellRequest),
    /// Terminal event observed by the permanent `UiRoot` subscription. The
    /// shell uses this as a redraw signal and the router resolves observed
    /// keyboard events centrally.
    TerminalEvent(TerminalObserverEvent),
}

/// Self-contained payload emitted by the permanent UiRoot terminal observer.
/// Otherwise unhandled events are represented without carrying a
/// framework-specific event payload because they only participate in redraw
/// accounting at the shell boundary. Mouse events are no longer observed here:
/// they reach components through `mouse_sub()` subscriptions and are arbitrated
/// by `sync_mouse_subscriptions` before delivery (ADR 0024).
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalObserverEvent {
    Key(TuiKeyEvent),
    Resize {
        width: u16,
        height: u16,
    },
    FocusGained,
    FocusLost,
    /// A left mouse button press, carried through so the shell can resolve
    /// it against shell-painted chrome (e.g. the tab bar, task 6.5) that has
    /// no mounted component of its own to claim it via `mouse_sub()`. Not a
    /// mouse-fold "claim" (it stays wrapped in `TerminalEvent`), so it never
    /// competes with a mounted component's own claim for the same click.
    MouseClick {
        column: u16,
        row: u16,
    },
    NoOp,
}

// TODO(migrate-tui-to-tuirealm): flesh out navigation targets as root/overlay
// routing converts (tasks 5.1/5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct NavTarget;
