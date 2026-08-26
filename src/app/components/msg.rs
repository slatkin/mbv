//! `Msg` and its request payloads (design D4).
//!
//! `Msg` carries only cross-authority requests; local state changes never
//! become a `Msg` (they mutate the component in `on`/`update` and return
//! `None`). Request payloads are placeholder scaffolds filled in as each
//! surface converts (see per-type TODOs).

use crossterm::event::{KeyEvent, MouseEvent};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueSlotId;

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

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackRequest {
    TogglePlayPause,
    Stop,
    Previous,
    Next,
    SeekRelative(i64),
    SeekTo(u16),
    ToggleMute,
    VolumeDelta(i64),
    CycleAudio,
    CycleSubtitle,
    ToggleVisualizer,
}

/// Queue requests carry slot identity, not a snapshot index. The queue can be
/// reordered by the Player between paint and dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum QueueRequest {
    Cursor {
        scope: crate::app::types_playback::QueueScope,
        slot_id: QueueSlotId,
    },
    Scope(crate::app::types_playback::QueueScope),
    Play {
        scope: crate::app::types_playback::QueueScope,
        slot_id: QueueSlotId,
    },
    Remove {
        scope: crate::app::types_playback::QueueScope,
        slot_id: QueueSlotId,
    },
    Move {
        scope: crate::app::types_playback::QueueScope,
        slot_id: QueueSlotId,
        direction: QueueMove,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueMove {
    Up,
    Down,
}

// TODO(migrate-tui-to-tuirealm): flesh out as service-driven surfaces convert
// (browse fetch / search / session / cast ops; tasks 3.x/4.x).
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceRequest {
    /// Dispatch a debounced search query to the Emby client. The shell owns
    /// the Emby client and spawns the search thread (task 3.2).
    SearchQuery(String),
    SettingsKey {
        cursor: usize,
        key: KeyEvent,
    },
    ActivateService(usize),
    SubmitEmbySetup {
        server_url: String,
        username: String,
        password: String,
    },
    SubmitAudiobookshelfSetup {
        server_url: String,
        api_key: String,
    },
    CancelSetup,
}

// TODO(migrate-tui-to-tuirealm): flesh out at Settings conversion (task 4.9).
#[derive(Debug, Clone, PartialEq)]
pub enum PersistRequest {
    SettingsKey { cursor: usize, key: KeyEvent },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumCursorKind {
    Move,
    Jump,
    Page,
}

/// Closed set of podcast show-list movement operations (task 5.3d.5). The
/// component performs its local cursor arithmetic and emits the matching
/// variant; the shell maps it onto the legacy App show-move operations
/// preserving the current position-save/detail-fetch target (D17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodcastShowMove {
    PreviousRow,
    NextRow,
    PreviousItem,
    NextItem,
    PreviousPage,
    NextPage,
    First,
    Last,
}

/// Closed set of podcast episode-mode transitions (task 5.3d.6). The
/// component performs its local episode/cursor/filter mutation and emits the
/// matching variant while episode selection is active; the shell maps it onto
/// the legacy App episode-move / filter-cycle / exit operations preserving the
/// current App episode target (D17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodcastEpisodeTransition {
    PreviousEpisode,
    NextEpisode,
    PreviousFilter,
    NextFilter,
    Exit,
}

/// Closed set of podcast episode action intents (task 5.3d.7). The component
/// emits the intent matched from Space/Enter/Ctrl+A; the shell resolves the
/// episode-selection and wide/narrow conditions from current App state/layout
/// at the Model boundary and runs the existing App effect (D17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodcastEpisodeIntent {
    /// Space: App enters episode selection when its episode selection is
    /// `None`; otherwise App plays its selected episode.
    FocusOrPlay,
    /// Enter: when App selection is `None`, wide podcast enters inline episode
    /// selection and narrow podcast opens the selection modal; otherwise App
    /// plays its selected episode.
    OpenOrPlay,
    /// Ctrl+A: enqueue only when App episode selection is active; otherwise
    /// no-op.
    Enqueue,
}
// TODO(migrate-tui-to-tuirealm): flesh out (mount/dismiss overlay, change
// focus, toast) as overlay routing converts (task 5.2).
/// Shell-level requests from Interactive Components: mount/dismiss overlays,
// quit, switch panels, toast. Fleshed out per-surface as components convert.
#[derive(Debug, Clone, PartialEq)]
pub enum ShellRequest {
    MusicAlbumCursor {
        target: usize,
        kind: AlbumCursorKind,
    },
    /// Activate the focused inline album track (Enter, or Ctrl+P while a
    /// track is focused): the shell resolves the track from
    /// `MusicWorkspaceComponent::track_cursor()` and plays it through the
    /// album queue path (`App::play_album_track`).
    MusicTrackActivate,
    /// Enqueue the focused inline album track (Ctrl+A while a track is
    /// focused): the shell resolves the track from the component cursor and
    /// enqueues it via the library-view enqueue path.
    MusicTrackEnqueue,
    /// Open the context menu targeted at the focused inline album track
    /// ('.' while a track is focused): the shell resolves the track item and
    /// raises the menu through `App` (target resolution lives at the
    /// shell/component boundary).
    MusicTrackContextMenu,
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
    /// Activate the context-menu entry at the component-owned cursor. The
    /// shell reads the entry's `ContextAction` and executes it (task 5.3c).
    ContextMenuSelect(usize),
    /// Dismiss the context menu (click outside / Esc-equivalent via mouse).
    ContextMenuDismiss,
    /// Dismiss the global Search sidebar (Esc or Backspace on empty query).
    DismissSearch,
    /// Activate the selected search result: navigate to the item. The shell
    /// owns the library tabs and navigation spawn; the component owns the
    /// cursor and results (task 3.2).
    SearchActivate {
        id: String,
        item_type: String,
    },
    /// Activate the selected item in an inline library Search component.
    InlineSearchActivate {
        id: String,
        item_type: String,
    },
    /// Open the inline library Search child for the focused Emby browser.
    OpenInlineSearch,
    /// Dismiss the focused inline library Search child.
    InlineSearchDismiss,
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
    /// action path, identified by its Feed guid.
    FeedsPlay(String),
    /// Enqueue the Feeds component's selected entry through the existing shell
    /// action path, identified by its Feed guid.
    FeedsEnqueue(String),
    /// Dismiss the blocking Selection modal.
    DismissSelectionModal,
    /// Select a source-specific filter in the Selection modal.
    SelectionModalFilterSelected,
    /// Rebuild a source-specific filter using the component-owned selection.
    SelectionModalRefresh,
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
    /// Continue Watching (Delete), keeping the cw-range guard the legacy
    /// Delete arm applied.
    HomeDelete(usize),
    /// Toggle the watched state of the Continue Watching column's own
    /// (independently tracked) cursor item -- Ctrl+W on Home. Matches the
    /// legacy `cw_toggle_watched`, which is not addressed by the Home flat
    /// cursor (preserved, not fixed).
    HomeToggleWatched,
    /// Persist the newly selected Home pill (section index) as the restored
    /// preference, resolved via the mounted component's `source_for_section`
    /// at the Model boundary (task 5.3d, numeric Home section deletion).
    HomeSectionSelected(usize),
    /// A Home-surface wheel scroll over the component's own list area
    /// (`list_area`, rebuilt every `view`; task 5.3d, home hit_test). The
    /// shell runs `App`'s 30ms wheel throttle and browse-readiness gate and
    /// then, at the Model boundary, moves the mounted component's
    /// section-local cursor plus the Continue Watching column's independent
    /// cursor (`Model::handle_home_scroll`, task 5.3d, Home wheel-scroll
    /// ownership); the component holds no timing state.
    HomeScroll {
        delta: i64,
    },
    /// A Home-surface click the component resolved to a region of its own
    /// geometry (task 5.3d, home hit_test). The component reports *where* it
    /// landed; the shell decides *when* it counts — it runs `App`'s 400ms
    /// double-click comparison against `App::last_click_time`/`last_click_pos`
    /// and then routes the region through `Model::handle_home_click` (task
    /// 5.3d, Home mouse-click handoff).
    HomeClick {
        region: HomeHitRegion,
        col: u16,
        row: u16,
    },
    /// Typed podcast show-list movement (task 5.3d.5). Emitted by the component
    /// after its local cursor mutation for Up/k, Down/j, Left/h, Right/l,
    /// PageUp/PageDown, Home/End while no episode selection is active; the
    /// shell maps the variant onto the legacy App show-move operations and
    /// re-projects podcast content.
    AudiobookshelfPodcastShowMove(PodcastShowMove),
    /// Typed podcast episode-mode transition (task 5.3d.6). Emitted by the
    /// component after its local episode-cursor/filter/exit mutation while
    /// episode selection is active (Up/k, Down/j, `[`, `]`, Esc, Backspace);
    /// the shell maps the variant onto the legacy App episode operations and
    /// re-projects podcast content.
    AudiobookshelfPodcastEpisodeTransition(PodcastEpisodeTransition),
    /// Typed podcast episode action intent (task 5.3d.7). Emitted by the
    /// component for Space/Enter/Ctrl+A; the shell resolves the episode-
    /// selection and wide/narrow conditions from current App state/layout and
    /// runs the existing App play/enter/modal/enqueue effect (D17).
    AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent),
    /// Forward Audiobookshelf book effects to the legacy App handler while
    /// the browser's local state remains component-owned.
    AudiobookshelfBookKey(crossterm::event::KeyEvent),
    /// Close the nested playlist view without dismissing the sidebar.
    PlaylistsBack,
    /// Load the selected playlist's items.
    PlaylistsOpen(usize),
    /// Play the selected playlist or item.
    PlaylistsActivate {
        open: bool,
        index: usize,
    },
    /// Open the rename dialog for the selected playlist.
    PlaylistsRename(usize),
    /// Ask for confirmation before deleting the selected playlist.
    PlaylistsDelete(usize),
    /// Refresh playlist data.
    PlaylistsRefresh,
    /// Dismiss the Playlists sidebar.
    DismissPlaylists,
    /// Dismiss the Settings sidebar, including nested settings destinations.
    DismissSettings,
    /// Forward Save-playlist effects to the legacy App handler while the
    /// dialog's local input remains component-owned.
    SavePlaylistKey(crossterm::event::KeyEvent),
    /// Forward queue keys whose effects are still shell-owned.
    QueueKey(crossterm::event::KeyEvent),
    /// A Queue-surface wheel scroll over the component's own list area
    /// (`area`, rebuilt every `view`; task 5.3d, queue hit_test). The shell
    /// runs `App`'s 30ms wheel throttle against `App::last_scroll_at` and
    /// then calls the extracted queue scroll gesture; the component holds no
    /// timing state.
    QueueScroll {
        delta: i64,
    },
    /// A Queue-surface click the component resolved to a region of its own
    /// geometry (task 5.3d, queue hit_test). The component reports *where*
    /// it landed; the shell decides *when* it counts via App's single
    /// double-click clock, then calls the extracted queue gesture method.
    QueueClick {
        region: QueueHitRegion,
        col: u16,
        row: u16,
    },
    /// A TV-workspace wheel scroll over the component's own series list
    /// (`layout.left_area`, the right-pane list area, rebuilt every `view`;
    /// task 5.3d, tv_workspace hit_test). The shell runs `App`'s 30ms wheel
    /// throttle against `App::last_scroll_at` and then calls
    /// `App::handle_mouse_scroll_browse`; the component holds no timing
    /// state.
    TvScroll {
        delta: i64,
    },
    /// A TV-workspace click the component resolved to a region of its own
    /// geometry (task 5.3d, tv_workspace hit_test). The component painted
    /// the two panes, so it resolves which pane a click lands in and the hit
    /// within it; the shell never re-derives the pane from `col`/`row`. The
    /// shell decides *when* it counts — via `App`'s 400ms double-click
    /// window (`note_browse_double_click`) — then calls the matching
    /// extracted gesture method.
    TvClick {
        region: TvHitRegion,
        col: u16,
        row: u16,
    },
    /// Browse-surface scroll over the browser list, hit-tested locally by
    /// `BrowserComponent` against its own `LayoutMain` (task 5.3d, browser
    /// hit_test). The shell runs `App`'s 30ms wheel throttle against
    /// `App::last_scroll_at` and then calls `App::handle_mouse_scroll_browse`;
    /// the component holds no timing state.
    BrowserScroll {
        delta: i64,
    },
    /// A browse-surface click the component resolved to a region of its own
    /// geometry (task 5.3d correction). The component reports *where* it
    /// landed; the shell decides *when* it counts — it runs `App`'s 400ms
    /// double-click comparison against `App::last_click_time`/`last_click_pos`
    /// and then calls the matching extracted gesture method
    /// (`handle_mouse_single_click_emby`, `handle_mouse_double_click_emby`,
    /// `handle_mouse_right_click_emby`, `handle_mouse_selector_click_emby`).
    BrowserClick {
        region: BrowserHitRegion,
        col: u16,
        row: u16,
    },
    /// Enter on the mounted generic/Movies/home-video `BrowserComponent`
    /// (task 5.3d, Emby browser effect decoupling): the component resolved
    /// its own selected `EmbyItem` from its component-local cursor/content,
    /// and the shell runs `App::select_item` on that supplied item directly
    /// (folder/library navigation and playable behavior preserved) — never
    /// by copying the component cursor into a `BrowseLevel.cursor` and
    /// re-reading it. The shell derives the active library index from its own
    /// tab state (the browser is mounted only for that tab).
    BrowserActivate {
        item: EmbyItem,
    },
    /// Ctrl+P on the mounted generic/Movies/home-video `BrowserComponent`
    /// (task 5.3d, Emby browser effect decoupling): the component resolves
    /// its selected item and the shell applies the preserved Ctrl+P tail to
    /// it — folder items play the folder through the collection queue source
    /// (`play_folder` + `save_queue_state`), non-folder items activate via
    /// `select_item` — acting on the supplied item, never an App cursor
    /// re-read.
    BrowserPlay {
        item: EmbyItem,
    },
    /// Ctrl+A on the mounted generic/Movies/home-video `BrowserComponent`
    /// (task 5.3d, Emby browser effect decoupling): the component resolves
    /// its selected item and the shell enqueues that supplied item through
    /// the existing item-targeted seam (`App::enqueue_lib_item`), preserving
    /// the folder/non-playable guards, route-conflict, local/remote queue,
    /// and reconciliation behavior.
    BrowserEnqueue {
        item: EmbyItem,
    },
    /// Ctrl+W on the mounted generic/Movies/home-video `BrowserComponent`
    /// (task 5.3d, Emby browser effect decoupling): the component resolves
    /// its selected item and the shell toggles that supplied item's watched
    /// state through `App::toggle_watched_item` — folder/audio guards, the
    /// mark played/unplayed API, unplayed-only/feed-home-video removal,
    /// refresh, and unavailable-Service/error toasts all preserved, acting
    /// on the supplied item identity (not the legacy `BrowseLevel.cursor`
    /// re-read).
    BrowserToggleWatched {
        item: EmbyItem,
    },
    /// '.' on the mounted generic/Movies/home-video `BrowserComponent`
    /// (task 5.3d, Emby browser context-menu decoupling): the component
    /// resolves its own selected `EmbyItem` from its component-local
    /// cursor/content, and the shell opens the context menu for that supplied
    /// item via the existing item-targeted seam `App::open_context_menu_for`
    /// — never by copying the component cursor into a `BrowseLevel.cursor`
    /// and re-reading it. The library/podcast menu content (mark-watched vs
    /// mark-played labels, bulk actions) derives from the shell's own tab
    /// state (the browser is mounted only for that tab).
    BrowserContextMenu {
        item: EmbyItem,
    },
    /// Ctrl+S on the mounted generic/Movies/home-video `BrowserComponent`
    /// (task 5.3d, Emby browser shuffle decoupling): the component resolves
    /// its own selected `EmbyItem` from its component-local cursor/content,
    /// and the shell shuffles that supplied item — the folder itself when it
    /// is a folder, otherwise the current browse level's parent (falling back
    /// to the library id) — via the preserved `shuffle_play` tail, never by
    /// re-reading `BrowseLevel.cursor`.
    BrowserShuffle {
        item: EmbyItem,
    },
    /// Bare or Alt+`r` on the mounted generic/Movies/home-video
    /// `BrowserComponent` (task 5.3d, Emby browser refresh): the component
    /// reports that the focused browser wants a metadata refresh, and the
    /// shell derives the active Emby library index from its own tab state and
    /// runs `App::refresh_lib` on it — preserving the bare-`r` *and* legacy
    /// Alt+`r` behavior (both reach the legacy arm without a CONTROL
    /// modifier). No item is carried: the effect targets the whole active
    /// library, not a selected row.
    BrowserRefresh,
    /// Ctrl+`r` on the mounted generic/Movies/home-video `BrowserComponent`
    /// (task 5.3d, Emby browser rescan): the component reports that the
    /// focused browser wants a metadata rescan, and the shell raises the same
    /// Rescan Library confirmation the legacy `handle_lib_key` arm did
    /// (identical title/message/hint and `ConfirmAction::RescanLibrary(lib_idx)`
    /// derived from the shell's own tab state). No item is carried: the rescan
    /// confirmation covers the whole active library.
    BrowserRescan,
    /// Esc or Backspace on the focused generic/Movies/home-video
    /// `BrowserComponent` (task 5.3d, Emby browser back): back-navigation
    /// moves off `Msg::Legacy`. The component emits `BrowserBack` for
    /// `KeyCode::Esc`/`KeyCode::Backspace` with any modifier (matching the
    /// legacy `handle_lib_key` arm, which guarded neither), and the shell
    /// derives the active Emby library index from its own tab state and runs
    /// `App::go_back` on it — preserving its synthetic-group/root guards,
    /// parent-cursor restoration, season-level skip, persistence, and
    /// stale-index behavior. No item is carried: back targets the browse
    /// history, not a selected row.
    BrowserBack,
    /// `[`/`]` on the focused generic/Movies/home-video `BrowserComponent`
    /// (task 5.3d, Emby browser selector cycling): the component reports the
    /// letter-range-pill cycle delta (-1 for `[`, +1 for `]`) with neither
    /// CONTROL nor ALT — exactly the legacy `handle_key_emby_library` guard —
    /// and the shell derives the active Emby library index from its own tab
    /// state and runs `App::cycle_letter_pill` on it. The component's mount
    /// gate already excludes Music and feed-home-video group views, so its
    /// bracket keys can only mean letter-pill cycling; `cycle_letter_pill`
    /// keeps its `should_show_letter_pills` no-op guard and wrap/select
    /// behavior. No item is carried: the pill row is a whole-library control,
    /// not a selected row.
    BrowserCycleLetterPill {
        delta: i64,
    },
    /// Up/Down/k/j/PageUp/PageDown on the focused generic/Movies/home-video
    /// `BrowserComponent` (task 5.3d, Emby browser local navigation): the
    /// component reports the display-row delta it already applied to its own
    /// cursor, and the shell derives the active Emby library index from its
    /// own tab state and runs `App::move_lib_cursor_rows` on it — the same
    /// method the legacy `handle_lib_key` movement arms call — so the App
    /// cursor mirrors the component through the typed path. The payload is
    /// display rows (Up/k `-1`, Down/j `1`, PageUp `-page_rows()`, PageDown
    /// `page_rows()`); the App method applies its own painted column count
    /// to stride, exactly like the legacy arm. Calling the App method (never
    /// a raw cursor-field write) preserves `save_default_library_position` /
    /// `mark_library_navigation` / `maybe_fetch_next_page` / `last_nav_at`
    /// idle side effects byte-for-byte. The legacy season-grid branch is
    /// unreachable here: the Browser mount gate excludes TV.
    BrowserMoveRows {
        rows: i64,
    },
    /// Left/Right/h/l on the focused generic/Movies/home-video
    /// `BrowserComponent` with a multi-column painted list (task 5.3d, Emby
    /// browser local navigation): the component reports the column delta it
    /// already applied to its own cursor, and the shell derives the active
    /// Emby library index from its own tab state and runs
    /// `App::move_lib_cursor` on it — the same method the legacy
    /// `handle_lib_key` column arms call (`-1` for Left/h, `1` for
    /// Right/l), preserving the same navigation side effects as
    /// `BrowserMoveRows`. A one-column list never emits this request: those
    /// keys stay unbound (matching legacy `handle_lib_key`'s 1-column
    /// guard) and the raw key still falls through to the legacy bridge.
    BrowserMoveColumn {
        delta: i64,
    },
    /// Home/End on the focused generic/Movies/home-video `BrowserComponent`
    /// (task 5.3d, Emby browser local navigation): the component reports the
    /// jump direction it already applied to its own cursor (`false` jumps to
    /// the first item, `true` to the last), and the shell derives the active
    /// Emby library index from its own tab state and runs
    /// `App::jump_lib_cursor` on it — the same method the legacy
    /// `handle_lib_key` Home/End arms call, preserving the same navigation
    /// side effects as `BrowserMoveRows`.
    BrowserJumpCursor {
        to_end: bool,
    },
}

/// Region of the generic Emby browser a click resolved to, reported by
/// `BrowserComponent` (task 5.3d correction). The shell turns this plus
/// `col`/`row` into the right gesture call; the component holds no double-click
/// or scroll timing state of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserHitRegion {
    /// Left list area; the component resolves the row via `left_row_map`.
    LeftRow(usize),
    /// Inline hero (browse surface for the two Services that publish one).
    InlineHero(usize),
    /// Selector-tab pill; `target` is the pill index the component resolved.
    SelectorTab(usize),
    /// Right-click → Emby context menu after the row is focused.
    ContextMenu(usize),
}

/// Region of the Home surface a click resolved to, reported by
/// `HomeComponent` (task 5.3d, home hit_test). The shell turns this plus
/// `col`/`row` into the right gesture call; the component holds no double-click
/// or scroll timing state of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeHitRegion {
    /// The Home list area (`list_area`): the component resolves the row
    /// under the click; the shell decides whether the same coordinates form
    /// a single click (focus Library) or a double-click activation of the
    /// resolved flat target.
    Row(usize),
    /// Section pill; `target` is the section index the component resolved.
    Pill(usize),
    /// Right-click → Home context menu after the row is focused.
    ContextMenu(usize),
}

/// Region of the Queue surface a click resolved to, reported by
/// `QueueComponent` (task 5.3d, queue hit_test). The shell turns this plus
/// `col`/`row` into the matching App gesture; the component holds no
/// double-click or scroll timing state of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueHitRegion {
    /// Queue list area: a single click selects/focuses via
    /// The resolved slot target is applied by the shell, while the shell decides whether the same
    /// coordinates form a double-click activation.
    Row(Option<QueueSlotId>),
    /// Local queue scope pill.
    ScopeLocal,
    /// Remote queue scope pill.
    ScopeRemote,
    /// Right-click in the queue list area.
    ContextMenu(Option<QueueSlotId>),
}

/// Pane + hit within the TV workspace a click resolved to, reported by
/// `TvWorkspaceComponent` (task 5.3d, tv_workspace hit_test). The TV
/// workspace has two focusable panes, so a click's meaning depends on which
/// pane it lands in: Episodes-pane hits (season pill, episode row, blank
/// hero space) move the component's local pane focus to `Episodes` and pull
/// App's panel focus to the Library; Series-pane hits move the component's
/// pane to `Series` and set the library cursor in App. The component painted
/// the panes, so it resolves both; the shell never re-derives the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvHit {
    /// Season pill in the Episodes pane; index resolved by the component.
    SeasonTab(usize),
    /// Episode row in the Episodes pane; index resolved by the component.
    EpisodeRow(usize),
    /// Blank/hero space in the Episodes pane (no tab or row under the
    /// cursor): consumed without changing the pane or panel focus.
    EpisodesPane,
    /// The Series pane (series list): the series row the component resolved
    /// from its own painted geometry. The shell sets `App`'s library cursor
    /// to `target` before any pane effect (activation, context menu).
    SeriesRow(usize),
}

/// Region of the TV workspace a click resolved to (task 5.3d, tv_workspace
/// hit_test). The component resolves the pane and the hit within it; the
/// shell turns the region into the matching App gesture — single vs
/// double-click decided there via App's 400ms window — without re-deriving
/// the pane from the click coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvHitRegion {
    /// A left click on the carried `TvHit`.
    Hit(TvHit),
    /// A right click; the carried `TvHit` is the pane + hit the click
    /// resolved to, so the shell applies the same pane-appropriate
    /// single-click effect (panel focus for Episodes-pane hits, series
    /// cursor for Series-pane hits) before opening the context menu at the
    /// click position.
    ContextMenu(TvHit),
}
