//! `Msg` and its request payloads (design D4).
//!
//! `Msg` carries only cross-authority requests; local state changes never
//! become a `Msg` (they mutate the component in `on`/`update` and return
//! `None`). Request payloads are placeholder scaffolds filled in as each
//! surface converts (see per-type TODOs).

use crossterm::event::{KeyEvent, MouseEvent};

/// The single TuiRealm outbound type, grouping surface output enums (design
/// D4). `Application` requires `Msg: PartialEq`; convenience `Debug`/`Clone`
/// derives aid diagnostics and follow-on message cascades.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Navigate(NavTarget),
    Playback(PlaybackRequest),
    Queue(QueueRequest),
    Service(ServiceRequest),
    Persist(PersistRequest),
    Shell(ShellRequest),
    /// Temporary adapter carrying a translated terminal event out of the
    /// `LegacyInput` bridge so the shell `Model` can re-run the existing
    /// `App` input handlers (design D11/D13). This is NOT a domain message:
    /// it exists only for the mixed-framework strangler phase and is removed
    /// at the completion gate.
    // TODO(migrate-tui-to-tuirealm): delete this variant at task 5.3 when
    // `LegacyInput` is removed and the last surface leaves the legacy path.
    Legacy(LegacyTerminalEvent),
}

/// Terminal-event payload for the temporary `Msg::Legacy` bridge (design D13).
///
/// `LegacyInput` reconstructs crossterm events from TuiRealm's `Event` and
/// carries them here so the `Model` can call the existing `App::handle_key` /
/// `App::handle_mouse` / focus / resize handlers unchanged. `Key`/`Mouse`
/// carry crossterm types (which impl `PartialEq`/`Clone`); `Resize` drops the
/// dimensions because the legacy resize handler ignores them (it only
/// force-clears and flushes image caches); `NoOp` covers events the legacy
/// loop no-ops (non-`Press` key kinds — which TuiRealm's crossterm adapter
/// collapses to `Event::None` — and `Paste`, which the legacy `_ => {}` arm
/// already ignored).
// TODO(migrate-tui-to-tuirealm): delete at task 5.3 with `Msg::Legacy`.
#[derive(Debug, Clone, PartialEq)]
pub enum LegacyTerminalEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize,
    FocusGained,
    FocusLost,
    NoOp,
}

// TODO(migrate-tui-to-tuirealm): flesh out navigation targets as root/overlay
// routing converts (tasks 5.1/5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct NavTarget;

// TODO(migrate-tui-to-tuirealm): flesh out at Playback chrome conversion
// (task 4.10); opaque QueueSlotId keys are resolved by the shell/Player.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackRequest;

// TODO(migrate-tui-to-tuirealm): flesh out at Queue conversion (task 4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct QueueRequest;

// TODO(migrate-tui-to-tuirealm): flesh out as service-driven surfaces convert
// (browse fetch / search / session / cast ops; tasks 3.x/4.x).
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceRequest {
    /// Dispatch a debounced search query to the Emby client. The shell owns
    /// the Emby client and spawns the search thread (task 3.2).
    SearchQuery(String),
}

// TODO(migrate-tui-to-tuirealm): flesh out at Settings conversion (task 4.9).
#[derive(Debug, Clone, PartialEq)]
pub struct PersistRequest;

// TODO(migrate-tui-to-tuirealm): flesh out (mount/dismiss overlay, change
// focus, toast) as overlay routing converts (task 5.2).
/// Shell-level requests from Interactive Components: mount/dismiss overlays,
// quit, switch panels, toast. Fleshed out per-surface as components convert.
#[derive(Debug, Clone, PartialEq)]
pub enum ShellRequest {
    /// Quit the application.
    Quit,
    /// Dismiss the Help overlay (Esc/F1 while help is open).
    DismissHelp,
    /// Switch from the current overlay to Settings (F2).
    OpenSettings,
    /// Switch from the current overlay to Sessions (F3).
    OpenSessions,
    /// Switch from the current overlay to Playlists (F4).
    OpenPlaylists,
    /// Forward a key to the shell's existing daemon-lost-modal handler. The
    /// DaemonLost component owns rendering and the blocking-modal swallow
    /// semantics; the shell owns restart/quit dispatch (which calls
    /// `restart_local_daemon`/`try_quit` — process-lifecycle effects that
    /// stay shell-owned).
    ConfirmKey(crossterm::event::KeyEvent),
    DaemonLostKey(crossterm::event::KeyEvent),
    /// Forward a key to the shell's existing remote-reanchor handler. The
    /// RemoteReanchor component owns rendering and the blocking-modal swallow
    /// semantics; the shell owns cursor/targets and the reconciliation effect.
    RemoteReanchorKey(crossterm::event::KeyEvent),
    /// Forward a key to the shell's existing context-menu handler. The
    /// ContextMenu component owns rendering and the blocking-modal swallow
    /// semantics; the shell owns cursor navigation and action execution.
    ContextMenuKey(crossterm::event::KeyEvent),
    /// Dismiss the global Search sidebar (Esc or Backspace on empty query).
    DismissSearch,
    /// Activate the selected search result: navigate to the item. The shell
    /// owns the library tabs and navigation spawn; the component owns the
    /// cursor and results (task 3.2).
    SearchActivate {
        id: String,
        item_type: String,
    },
    /// Close the Sessions sidebar without changing the selected destination.
    DismissSessions,
    /// Refresh the Emby session and Cast receiver snapshots.
    RefreshSessions,
    /// Activate the session/cast row at the component-owned cursor.
    SelectSession(usize),
    /// Detach the current session/cast playback target.
    DetachSessions,
    /// Refresh the Feeds subscriptions through the shell-owned worker.
    RefreshFeeds,
    /// Play the Feeds component's selected entry through the existing shell
    /// action path.
    FeedsPlay(usize),
    /// Enqueue the Feeds component's selected entry through the existing shell
    /// action path.
    FeedsEnqueue(usize),
    /// Dismiss the blocking Selection modal.
    DismissSelectionModal,
    /// Select a source-specific filter in the Selection modal.
    SelectionModalFilterSelected(usize),
    /// Activate the selected Selection modal item by its opaque provider id.
    SelectionModalActivate(Option<String>),
    /// Commit the component-owned Multiselect choices through the legacy App
    /// action path.
    MultiselectCommit {
        kind: crate::app::types_context_menu::MultiSelectKind,
        items: Vec<(String, String, bool)>,
    },
    /// Advance or leave the nested Library-routes picker through App's
    /// existing service/config action path.
    LibraryRoutesEnter,
    LibraryRoutesEsc,
    /// Forward a Feed-management action after the component has synchronized
    /// its local draft into the shell snapshot.
    FeedsManageKey(crossterm::event::KeyEvent),
    /// Forward a playback prompt key to the shell-owned Player handler.
    PlaybackPromptKey(crossterm::event::KeyEvent),
    /// Play the Home item at the component-owned flat cursor (task 3.4).
    HomePlay(usize),
    /// Enqueue the Home item at the component-owned flat cursor.
    HomeEnqueue(usize),
    /// Remove the Home item at the component-owned flat cursor from
    /// Continue Watching (Delete), matching the legacy `handle_cw_key`
    /// Delete arm's cw-range guard.
    HomeDelete(usize),
    /// Toggle the watched state of the Continue Watching column's own
    /// (independently tracked) cursor item -- Ctrl+W on Home. Matches the
    /// legacy `cw_toggle_watched`, which is not addressed by the Home flat
    /// cursor (preserved, not fixed).
    HomeToggleWatched,
    /// Persist the newly selected Home pill (section index) as the restored
    /// preference, matching `home_select_section`'s `save_prefs` call.
    HomeSectionSelected(usize),
    /// Forward Audiobookshelf podcast effects to the legacy App handler while
    /// the browser's local state remains component-owned.
    AudiobookshelfPodcastKey(crossterm::event::KeyEvent),
    AudiobookshelfPodcastMouse(crossterm::event::MouseEvent),
}
