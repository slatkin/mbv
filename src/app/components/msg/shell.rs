//! `ShellRequest` — the main shell-level request enum from Interactive
//! Components. Split from `msg.rs` (task 8.3) to keep the central `Msg` file
//! below the 800-line cap.
//!
//! Carries: mount/dismiss overlays, quit, panel switches, semantic intent
//! wrappers, and the typed geometry-aware mouse/click/scroll requests for
//! Home, Queue, TV, and Browser. The component owns the cursor and resolves
//! the click region; the shell owns the matching `App` side effect.

use mbv_core::api::EmbyItem;

use super::hit_regions::{BrowserHitRegion, HomeHitRegion, QueueHitRegion, TvHitRegion};
use super::intents::{
    AlbumCursorKind, AudiobookshelfBookIntent, AudiobookshelfBookMove, ConfirmIntent,
    ContextMenuIntent, DaemonLostIntent, FeedsManageIntent, PodcastEpisodeIntent,
    PodcastEpisodeTransition, PodcastShowMove, RemoteReanchorIntent, SavePlaylistIntent,
    SettingsIntent,
};
use super::queue::QueueIntent;

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
    /// Activate the selected album in narrow mode, where album tracks use the
    /// selection modal instead of the inline workspace.
    MusicAlbumActivate,
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
    /// Semantic confirmation intent; the component owns key interpretation and
    /// the shell owns the pending action's effect.
    ConfirmIntent(ConfirmIntent),
    /// Semantic daemon-lost intent; process-lifecycle effects remain shell-owned.
    DaemonLostIntent(DaemonLostIntent),
    /// Semantic remote-reanchor intent; cursor movement remains component-owned.
    RemoteReanchorIntent(RemoteReanchorIntent),
    /// Semantic context-menu intent; the component owns key interpretation.
    ContextMenuIntent(ContextMenuIntent),
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
    /// Semantic feed-management action; local form edits stay in the component.
    FeedsManageIntent(FeedsManageIntent),
    /// Play the Home item at the component-owned flat cursor (task 3.4).
    HomePlay(usize),
    /// Enqueue the Home item at the component-owned flat cursor.
    HomeEnqueue(usize),
    /// Open Home's context menu for the Continue Watching target resolved by
    /// the mounted component and its Model-owned content snapshot.
    HomeContextMenu {
        home_cw_selected: bool,
        cw_item: Option<mbv_core::api::EmbyItem>,
    },
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
    /// Typed Audiobookshelf book browser movement (task 5.3d.13-R1). The
    /// component updates its local browse state and the shell applies the
    /// corresponding legacy App operation, preserving position/detail effects.
    AudiobookshelfBookMove(AudiobookshelfBookMove),
    /// Typed Audiobookshelf book action (task 5.3d.13-R1). The shell resolves
    /// narrow/wide activation from current App state as the legacy reader did.
    AudiobookshelfBookIntent(AudiobookshelfBookIntent),
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
    /// Semantic save-playlist action; local text editing stays in the component.
    SavePlaylistIntent(SavePlaylistIntent),
    QueueIntent(QueueIntent),
    /// A Queue-surface wheel scroll over the component's own list area
    /// Semantic settings navigation/action intent; local cursor state stays
    /// in the Settings component.
    SettingsIntent(SettingsIntent),
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
    /// Series-list row movement from the TV workspace. The component applies
    /// the same local cursor delta before handing the App-side mirror update
    /// to the shell; episodes use `TvEpisodeMove` instead.
    TvMoveRows {
        rows: i64,
    },
    /// Left/right TV pane navigation. Wide TV is a one-column App list, so
    /// the shell intentionally treats this as a no-op after the component
    /// changes its local pane.
    TvMoveColumn {
        delta: i64,
    },
    /// Home/End movement in the TV series list.
    TvJumpCursor {
        to_end: bool,
    },
    /// Enter on a series row starts the component's episode-selection pane.
    /// Carries the component-resolved Series item so the shell effect never
    /// consults the (mirrored) App browse cursor.
    TvActivate {
        item: EmbyItem,
    },
    /// Enter on the focused TV episode pane plays the component-selected
    /// episode through the existing playback path.
    TvEpisodeActivate,
    /// Esc/Backspace leaves TV selection/back-navigates the App browse stack.
    TvBack,
    /// Series-root `[`/`]` cycle the App-owned letter pill.
    TvCycleLetterPill {
        delta: i64,
    },
    /// Episode cursor movement is local to the TV component; the shell keeps
    /// the typed request as an explicit no-op until the episode effect row.
    TvEpisodeMove {
        delta: i64,
    },
    /// Season cycling is local to the TV component; episode effects are a
    /// later typed-key slice.
    TvSeasonMove {
        delta: i64,
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
    /// moves off raw terminal forwarding. The component emits `BrowserBack` for
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
    /// Every local browser cursor key (arrows/hjkl, Page keys, Home/End) on
    /// the focused generic/Movies/home-video `BrowserComponent` (task 5.3d,
    /// Emby browser local navigation): the component resolves the target item
    /// index against its own painted geometry and reports it here. The shell
    /// applies the resolved index through the App nav level only to retain
    /// App-owned effects (`save_default_library_position` /
    /// `mark_library_navigation` / `maybe_fetch_next_page` / `last_nav_at`);
    /// it never recomputes the movement from a delta. The legacy season-grid
    /// branch is unreachable here: the Browser mount gate excludes TV.
    BrowserCursorIndex {
        index: usize,
    },
}
