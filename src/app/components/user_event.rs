//! `UserEvent` and its completion tokens (design D5).
//!
//! `UserEvent` carries a small `Eq`/`Clone` token identifying a completion;
//! the owned presentation model is pushed directly into the mounted target by
//! the shell via `get_component_mut`+downcast (design D5). Token payloads are
//! fleshed out here (task 1.6) but not yet injected by the shell; per-surface
//! conversion (tasks 3.x/4.x) wires each token as its component appears.
//!
//! # Shell-side adapter pattern
//!
//! For each run-loop receiver, the shell drains the channel, validates the
//! token's stale-completion guard (generation/key/revision), and — if not
//! stale — writes the validated payload into the target component via
//! `get_component_mut(id)?.as_any_mut().downcast_mut::<T>()`. The token itself
//! only carries the minimal `Eq`/`Clone` data the shell needs for the
//! stale-completion check; the rich presentation model never rides on the
//! token. Actual token injection is wired per-surface as components appear
//! (tasks 3.x/4.x); during CP1 no receiver emits a token yet.

use super::component_id::BrowserKey;
use std::time::Instant;

/// TuiRealm user-event type (design D5). `Application` requires `UserEvent:
/// Eq + PartialEq + Clone + Send + 'static`; the convenience `Debug` derive
/// aids diagnostics. `Clock` reuses `std::time::Instant` (which is `Eq`).
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum UserEvent {
    Startup(StartupTick),
    LibraryReady(BrowserKey, Generation),
    SearchReady(SearchGen),
    Session(SessionGen),
    Cast(CastGen),
    SharedData(SharedRev),
    Feed(FeedKey, Generation),
    Image(ImageKey),
    Websocket(WsTick),
    AbsSocket(AbsTick),
    Clock(Instant),
}

/// Setup-generation tick carried by `Startup` (design Table A rows 1-2).
///
/// Fleshed out (task 1.6); not yet injected by the shell startup/setup
/// receivers — wired at per-surface conversion (tasks 3.x/4.x).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StartupTick {
    /// Setup generation counter; the shell compares this against the current
    /// startup generation to discard stale completions.
    pub generation: u64,
}

/// Stale-completion generation carried by `LibraryReady` and `Feed` (design D5).
///
/// Fleshed out (task 1.6); not yet injected by the shell library/feed
/// receivers — wired at per-surface conversion (tasks 3.5/3.6).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Generation {
    /// Library/feed generation counter; the shell compares this against the
    /// owning browser/feed generation to discard stale completions.
    pub generation: u64,
}

/// Search-query stale-check generation carried by `SearchReady`
/// (design Table A row 8).
///
/// Fleshed out (task 1.6); not yet injected by the shell search receiver —
/// wired at Search conversion (tasks 3.2/3.3).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SearchGen {
    /// Search query string; the shell compares this against the current query
    /// to discard results for a superseded search.
    pub query: String,
}

/// Session-poll generation carried by `Session` (design Table A row 9).
///
/// Fleshed out (task 1.6); not yet injected by the shell session receiver —
/// wired at Sessions conversion (task 3.7).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SessionGen {
    /// Session poll generation counter; the shell compares this against the
    /// current session generation to discard stale polls.
    pub generation: u64,
}

/// Cast generation carried by `Cast` (design Table A row 10).
///
/// Fleshed out (task 1.6); not yet injected by the shell cast receiver —
/// wired at Sessions conversion (task 3.7).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CastGen {
    /// Cast generation counter; the shell compares this against the current
    /// cast generation to discard stale cast events.
    pub generation: u64,
}

/// Shared-data revision carried by `SharedData` (design Table A row 11).
///
/// Fleshed out (task 1.6); not yet injected by the shell shared-data
/// receiver — wired at per-surface conversion (tasks 3.x/4.x).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SharedRev {
    /// Shared revision counter; the shell compares this against the last-applied
    /// shared revision to discard stale shared-data updates.
    pub rev: u64,
}

/// Feed key identifier carried by `Feed` (design Table A rows 12-13, 19).
///
/// Fleshed out (task 1.6); not yet injected by the shell feed receivers —
/// wired at Feeds conversion (task 3.6).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FeedKey {
    /// Feed key identifier; the shell uses this to route the completion to the
    /// owning feed and compare against the feed generation to discard stale
    /// results.
    pub key: String,
}

/// Image cache key / item id carried by `Image` (design Table A rows 14-16).
///
/// Fleshed out (task 1.6); not yet injected by the shell image-cache
/// receivers — wired at per-surface conversion (tasks 3.x/4.x).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ImageKey {
    /// Image cache key / item id; the shell uses this to route the image
    /// payload into the target component and discard stale fetches.
    pub key: String,
}

/// Websocket tick counter carried by `Websocket` (design Table A row 17).
///
/// Fleshed out (task 1.6); not yet injected by the shell websocket receiver —
/// wired at Playback-chrome conversion (task 4.10).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WsTick {
    /// Websocket tick counter; the shell uses this to order/discard stale
    /// websocket completions.
    pub tick: u64,
}

/// ABS socket tick counter carried by `AbsSocket` (design Table A rows 3, 18).
///
/// Fleshed out (task 1.6); not yet injected by the shell ABS receiver —
/// wired at ABS conversion (tasks 4.5/4.6).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AbsTick {
    /// ABS socket tick counter; the shell uses this to order/discard stale
    /// ABS-socket completions.
    pub tick: u64,
}
