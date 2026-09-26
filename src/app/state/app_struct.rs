use crate::app::infra::layout;
use crate::app::render;
use crate::app::state::panel_targets::PanelTarget;
use crate::app::state::queue_owner::QueueEpoch;
use crate::app::state::types::browse::{AlbumIndexState, SeriesDetail};
use crate::app::state::types::cast::CastAttachment;
use crate::app::state::types::confirm::ConfirmModal;
use crate::app::state::types::events::{PendingSeriesHandoff, PendingSeriesLanding};
use crate::app::state::types::feed::IdleFeed;
use crate::app::state::types::feed::SavePlaylistDialog;
use crate::app::state::types::feed_tab::FeedTabState;
use crate::app::state::types::library_tab::LibraryTab;
use crate::app::state::types::playback::{
    PendingQueueAction, PlaylistMutationState, QueueScope, ReplacementExecutor,
    SuspendedLocalSession, UndoEntry,
};
use crate::app::state::types::player_tab::PlayerTab;
use crate::app::state::types::settings::{PanelFocus, PanelMode, SettingsDestination};
use crate::app::state::types::tab_selection::TabSelection;
use crate::app::SidebarId;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueSlotId;
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_core::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use mbv_visualizer::{PipeWireWorker, StereoSampleWindow};
use mbv_ws::WsEvent;
use std::sync::mpsc;
use std::time::Instant;

/// Lifecycle of the one background album-artist request per music level
/// (design D4 of `fix-music-artist-resolution-batching`). `Failed` levels are
/// not terminal across candidates: the next candidate creation for the level
/// retries, and unresolved albums settle via `SETTLE_WINDOW` regardless.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum LevelFillState {
    /// A level fill is in flight. Set by `spawn_level_artist_fetch`.
    /// `orphan_risk` is true for a warm-up fill that lacked the level's album
    /// paths, so a later browse can perform one path-aware upgrade.
    Loading { orphan_risk: bool },
    /// The level's albums were bulk-filled into `album_artist_cache`.
    /// Warm-up fills carry orphan risk because they had no album paths for
    /// attributing nested-disc track buckets.
    Filled { orphan_risk: bool },
    /// The level fill failed (HTTP error or no tracks); albums resolve via
    /// the settle/fallback path.
    Failed,
}

/// The single "should a level fill start?" decision both fill entry points
/// consume — `start_or_supersede_music_grouping`'s dedupe and
/// `spawn_level_artist_fetch`'s guard — so the two interpretations of a
/// `LevelFillState` can never diverge again.
pub(in crate::app) enum LevelFillAction {
    /// A fill is already in flight or has completed for this level.
    NoWork,
    /// No fill has run for this level, or the last one failed.
    Request,
}

impl LevelFillState {
    /// Unified interpretation of a level-fill state (design D4):
    /// `Loading`/`Filled` levels do no work; a fresh or `Failed` level
    /// (re)starts the fill.
    pub(in crate::app) fn action_for(state: Option<&LevelFillState>) -> LevelFillAction {
        match state {
            Some(LevelFillState::Loading { .. } | LevelFillState::Filled { .. }) => {
                LevelFillAction::NoWork
            }
            Some(LevelFillState::Failed) | None => LevelFillAction::Request,
        }
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "26 independent core app-state bits spanning launch/readiness, load-in-flight, persistence/UI options, and transient connection flags across unrelated subsystems; grouping would be a pure lint dodge (design analysis, issue #804)"
)]
pub struct App {
    /// General application configuration is independent of the optional Emby
    /// runtime. Feed management reads and mutates this context directly.
    pub(in crate::app) config: std::sync::Arc<std::sync::Mutex<crate::config::Config>>,
    pub(in crate::app) emby_runtime: EmbyRuntime,
    pub(in crate::app) audiobookshelf_runtime: AudiobookshelfRuntime,
    pub(in crate::app) setup: crate::app::state::service_setup::ServiceSetup,
    pub(in crate::app) audiobookshelf_libraries:
        Vec<mbv_core::audiobookshelf::AudiobookshelfLibrary>,
    /// Most-recent `Newest Episodes` shelf per podcast library (async shelf
    /// fetch, Task 6.2), keyed by library id. `fetch_home()` rebuilds Home's
    /// Audiobookshelf Latest pills from this cache — never a blocking network
    /// call — and the shelf-fetch handler refreshes it (Task 6.3).
    pub(in crate::app) audiobookshelf_shelf_cache:
        std::collections::HashMap<String, Vec<mbv_core::playback_queue::QueueItem>>,
    pub(in crate::app) audiobookshelf_browse:
        Vec<crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseState>,
    pub(in crate::app) audiobookshelf_book_browse:
        Vec<crate::app::state::types::audiobookshelf_browse::AudiobookshelfBookBrowseState>,
    pub(in crate::app) player: PlayerProxy,
    /// Bare mode's owner-side transition state. Remote targets use their
    /// daemon-owned coordinator; this is still hosted here so local jumps
    /// receive the same request identity semantics.
    pub(in crate::app) bare_owner: mbv_core::player_owner_state::PlayerOwnerState,
    /// Handle to the live MPRIS D-Bus registration, if one was started for
    /// this session (`App::new` / `App::new_remote` both start one; test
    /// construction via `build()` does not). `None` in tests so they never
    /// spin up a real D-Bus connection.
    ///
    /// `switch_to_direct_remote` and `restore_local_mode` call
    /// `mpris::rebind` on this whenever they swap `player` between a local
    /// `Player` and a `RemotePlayer` (#175): MPRIS must always publish
    /// whichever one currently owns playback, not whatever was live when
    /// the D-Bus service was first registered.
    pub(in crate::app) mpris: Option<crate::mpris::MprisHandle>,
    pub(in crate::app) player_rx: mpsc::Receiver<PlayerEvent>,
    pub(in crate::app) ws_rx: mpsc::Receiver<WsEvent>,
    pub(in crate::app) audiobookshelf_socket_rx:
        mpsc::Receiver<mbv_core::audiobookshelf::socket::SocketEvent>,
    pub(in crate::app) audiobookshelf_socket_tx: Option<mpsc::Sender<()>>,
    pub(in crate::app) audiobookshelf_socket_generation:
        Option<mbv_core::service_runtime::SetupGeneration>,
    pub(in crate::app) libs: Vec<LibraryTab>,
    pub(in crate::app) player_tab: PlayerTab,
    pub(in crate::app) remote_player_tab: Option<PlayerTab>,
    pub(in crate::app) status: String,
    pub(in crate::app) status_expires: Option<Instant>,
    pub(in crate::app) status_severity: crate::app::dispatch::notify::ToastSeverity,
    /// `true` only for instances built via `App::new_remote` (the
    /// `--connect-daemon` / local-daemon-auto-detect thin-client launch
    /// path). Those instances never populate `active_route` or
    /// `connected_session_state` (those are set by runtime library-route
    /// switches / session attaches that only apply to `App::new` instances),
    /// so `teardown`'s auto-reconnect persistence (#236) must skip this flag
    /// entirely rather than compute (and save) a bogus `None` record that
    /// would wipe out a real record saved by a different `App::new` session.
    pub(in crate::app) launched_as_remote: bool,
    /// The daemon endpoint for the current player target. `None` means an
    /// in-process player (bare mode). `Some(DaemonEndpoint::Local)` is this
    /// machine's managed local daemon. `Some(Tcp | Unix)` is a different
    /// daemon. Replaces the mutable `is_local_daemon` boolean so every
    /// transition records its source of truth rather than projecting it
    /// down to a bool that must be manually kept in sync.
    pub(in crate::app) player_endpoint: Option<mbv_core::remote_player::DaemonEndpoint>,
    /// The one-time, launch-time launch classification: `true` only for
    /// `App::new_remote` instances constructed for the managed local
    /// daemon, and never updated afterward. Kept independent of
    /// `player_endpoint`, which tracks the *current* player target and can
    /// change at runtime. Kept fixed at its construction-time value so
    /// `restore_local_mode` can tell whether this app's baseline (the state
    /// to return to when a route switch is undone) was a genuinely local
    /// in-process player (nothing to do here) or a connection to the local
    /// daemon (which must be reconnected, since there's no suspended local
    /// player to restore in that case).
    pub(in crate::app) home_is_local_daemon: bool,
    pub(in crate::app) hidden_libraries: Vec<String>,
    pub(in crate::app) home_latest_launch_window:
        crate::app::state::home_latest::HomeLatestLaunchWindow,
    /// `Config.library_routes` at startup (#256). Values are resolved
    /// `tcp://host:port` endpoints, read directly with no live-session
    /// lookup -- see `mbv_core::config::resolve_library_route`.
    pub(in crate::app) library_routes: std::collections::BTreeMap<String, String>,
    pub(in crate::app) music_levels: Vec<String>,
    pub(in crate::app) album_indexes: std::collections::HashMap<String, AlbumIndexState>,
    // Per-frame layout geometry from last render, used for mouse hit-testing.
    // See src/app/layout.rs for the grouping rationale.
    pub(in crate::app) layout: layout::AppLayout,
    pub(in crate::app) terminal_width: u16,
    pub(in crate::app) terminal_height: u16,
    /// Shell handoff for a modal raised by App-owned effects. The mounted
    /// component owns the modal after the next Model tick.
    pub(in crate::app) pending_overlay: Option<crate::app::state::types::overlay::OverlayRequest>,
    /// Set right before requesting a clean exit on an announced daemon
    /// shutdown (task 7.2); printed once by `run()` after the terminal is
    /// restored, since anything written while still in the alternate screen
    /// would never be visible. `None` on every other exit path.
    pub(in crate::app) pending_exit_message: Option<String>,
    pub(in crate::app) pending_delete_slot: Option<QueueSlotId>, // marks a delete that was already applied optimistically, so the Stopped handler doesn't re-derive it
    pub(in crate::app) queue_undo_stack: Vec<UndoEntry>,
    pub(in crate::app) remote_queue_undo_stack: Vec<UndoEntry>,
    pub(in crate::app) pending_remote_move_cursor: Option<usize>,
    /// The display cursor a just-issued local queue edit (e.g. remove) wants
    /// the next `UnifiedQueueUpdated` broadcast to land on, since the daemon's
    /// state tracks *playback* position, not the UI selection — see
    /// `remove_from_queue` and `PlayerEvent::UnifiedQueueUpdated`.
    pub(in crate::app) pending_queue_edit_cursor: Option<usize>,
    /// One-shot cursor re-anchor for the next queue sync.
    pub(in crate::app) pending_queue_cursor_reanchor: Option<QueueScope>,
    pub(in crate::app) next_up_item: Option<EmbyItem>,
    // Main UI scalars.
    // reuses shared self.libs.
    pub(in crate::app) panel_focus: PanelFocus,
    // Ephemeral narrow-terminal (< MINI_VIEW_THRESHOLD columns) two-state
    // toggle target: Library ⇄ Queue. Never read from or written to prefs;
    // tracked independently so wide-mode `panel_mode`/`panel_focus` stay
    // untouched while narrow. Defaults to Queue.
    pub(in crate::app) mini_view_focus: PanelFocus,
    pub(in crate::app) tab: TabSelection, // which left-panel tab is active
    pub(in crate::app) queue_column_width: u16,
    /// Persisted Wide hero list-pane width override; `None` = the shared
    /// arrangement's default ~40% ratio. One width is shared by every Wide
    /// hero surface and clamped against each surface's active content-area
    /// width at paint time (`list_pane_width`).
    pub(in crate::app) list_pane_width: Option<u16>,
    pub(in crate::app) visual_slot_hidden: bool,
    pub(in crate::app) panel_mode: PanelMode,
    pub(in crate::app) library_tab_pending: usize, // restored from prefs; applied once libs have loaded
    /// The one startup launch snapshot loaded from disk. Its tab identity is
    /// the sole source for tab restoration; `pending_launch_tab_resolved`
    /// consumes only that level while selector/item identities remain pending
    /// for the selected destination's discrete re-anchor.
    pub(in crate::app) pending_launch_state: Option<mbv_core::config::TuiLaunchState>,
    pub(in crate::app) pending_launch_tab_resolved: bool,
    /// Legacy selected-tab preference retained only until its stable identity
    /// can be recovered from the current catalogs. It is never used as a
    /// launch-state identity after migration.
    pub(in crate::app) legacy_launch_tab: Option<usize>,
    /// Prevents a legacy seed from being derived more than once in this
    /// process. The legacy files remain read-only compatibility inputs; the
    /// orderly-exit snapshot makes the new file authoritative.
    pub(in crate::app) legacy_launch_migration_attempted: bool,
    pub(in crate::app) emby_catalog_ready: bool,
    pub(in crate::app) audiobookshelf_catalog_ready: bool,
    /// Deferred tab switch for a `NavigateLanding::Album` landing (design D4
    /// of change `per-destination-item-navigation`): set when the recursive
    /// album activation spawns, consumed on its `RecursiveAlbumActivated`
    /// drain once the landed nav stack has replaced the saved position, so
    /// the switch's activation never restores a stale position. Consumed only
    /// when the drained activation belongs to the pending library; any other
    /// library's activation leaves it armed. Loss semantics: it is cleared by
    /// any manual tab change (`apply_tab_position`) and by `LibEvent::Error`,
    /// so a user who moved on or a failed activation never gets yanked to the
    /// navigated library (U2 correction).
    pub(in crate::app) pending_navigate_tab_switch: Option<usize>,
    /// Ensure-then-land state for a `NavigateLanding::Series` whose target
    /// library corpus is not ready (U2 correction): armed by the
    /// `NavigateTo` arm, retried on that library's `Loaded`,
    /// `AllItemsPrefetched` and restored-position drains. See
    /// `PendingSeriesLanding` for the full lifecycle.
    pub(in crate::app) pending_series_landing: Option<PendingSeriesLanding>,
    /// A landing that completed and still owes the shell's Series detail
    /// hand-off (task 3.1, design D3): armed by the landing success path and
    /// consumed by the shell's sync pass, which runs the Inline Search series
    /// presentation sequence. The deferred `PendingSeriesLanding` retry arms
    /// it the same way, so the hand-off fires at the actual landing
    /// completion, not at the original `NavigateTo` drain.
    pub(in crate::app) pending_series_handoff: Option<PendingSeriesHandoff>,
    /// Deep selection (task 6.2, design D6 of change
    /// `per-destination-item-navigation`): the chosen track from a
    /// `NavigateLanding::Album`, armed when the recursive album activation
    /// spawns and bound to the activated album at the shell's
    /// `RecursiveAlbumActivated` drain. Keyed by the target library so a
    /// foreign library's activation never adopts it.
    pub(in crate::app) pending_track_selection: Option<(usize, String)>,
    pub(in crate::app) last_played_item_id: Option<String>,
    pub(in crate::app) last_played_completed: bool,
    /// The queue visual slot's image projection (task 3.4, D9): the queue
    /// projection issues every fetch for the now-playing item and projects
    /// the slot's image state; the painter reads this and paints. Refreshed
    /// by `Model::sync_queue`'s push while playback is active.
    pub(in crate::app) queue_card_projection:
        crate::app::render::components::card::QueueCardProjection,
    pub(in crate::app) dim_backdrop_active: bool,
    pub(in crate::app) settings_destination: SettingsDestination,
    pub(in crate::app) settings_save_at: Option<Instant>,
    /// Live mouse-capture flip requested by the Settings toggle arm (ADR
    /// 0024): the run loop consumes it and applies `set_mouse_capture` on
    /// the session stdout once per tick.
    pub(in crate::app) mouse_capture_pending: Option<bool>,
    pub(in crate::app) confirm_logout: bool,
    pub(in crate::app) system_notifications: bool,
    pub(in crate::app) notif_failed: bool,
    pub(in crate::app) channels: crate::app::state::runtime_channels::RuntimeChannels,
    /// Whether the global Search sidebar overlay is open. The
    /// `SearchSidebarComponent` owns the sidebar state (query, cursor, scroll,
    /// results, debounce); this flag tells the legacy render/input path the
    /// overlay is active (task 3.2).
    pub(in crate::app) sessions: Vec<mbv_core::api::SessionInfo>,
    /// Last cast discovery browse result (8.1), independent of `sessions`'s
    /// own reload cadence -- see `panel_targets::build_panel_targets`.
    pub(in crate::app) cast_receivers: Vec<mbv_core::cast::discovery::CastReceiver>,
    /// The F3 panel's merged Emby+Cast target list, rebuilt from `sessions`/
    /// `cast_receivers` by `App::rebuild_panel_targets` (8.1/8.2).
    pub(in crate::app) panel_targets: Vec<PanelTarget>,
    pub(in crate::app) sessions_loading: bool,
    pub(in crate::app) playlists: Vec<EmbyItem>,
    pub(in crate::app) playlists_cursor: usize,
    pub(in crate::app) playlists_scroll: usize,
    pub(in crate::app) playlists_loading: bool,
    pub(in crate::app) playlists_open: Option<EmbyItem>, // playlist currently being browsed
    pub(in crate::app) playlists_open_items: Vec<EmbyItem>,
    pub(in crate::app) playlists_open_cursor: usize,
    pub(in crate::app) playlists_open_scroll: usize,
    pub(in crate::app) playlists_open_loading: bool,
    pub(in crate::app) queue_source: crate::config::QueueSource,
    pub(in crate::app) queue_dirty: bool,
    pub(in crate::app) pending_owner_source_update:
        Option<(crate::config::QueueSource, mbv_core::ctrl::QueueLineage)>,
    /// Deferred queue replacement awaiting the save/discard answer, then the
    /// `PlaylistMutationComplete` boundary. Owned by `replace_queue_or_prompt`
    /// and its existing callers (`ClearQueue`, the album/artist track paths,
    /// the notification/quit auto-save routes).
    pub(in crate::app) pending_queue_action: Option<PendingQueueAction>,
    /// A resolved, not-yet-confirmed queue replacement held by the D6
    /// populated-queue gate (`ConfirmAction::ReplacePopulatedQueue`). The
    /// tuple pairs the action with the executor that runs it once confirmed,
    /// so a `Routed` entry point replays its own prep instead of the wrong
    /// path. This is the gate's own slot: its only reader is that confirmation
    /// arm, so the save-deferral boundary
    /// (`SessionEvent::PlaylistMutationComplete`) can never pick a gated
    /// payload up and execute it unconfirmed.
    pub(in crate::app) pending_queue_replacement: Option<(PendingQueueAction, ReplacementExecutor)>,
    /// Deferred explicit play awaiting the section-5 local fall-through prompt.
    pub(in crate::app) pending_local_play: Option<PendingQueueAction>,
    pub(in crate::app) use_nerd_fonts: bool,
    pub(in crate::app) indicator_style: render::indicators::IndicatorStyle,
    pub(in crate::app) ws_send_tx: Option<mbv_ws::WsSender>,
    pub(in crate::app) last_keepalive: Instant,
    pub(in crate::app) last_capabilities: Instant,
    pub(in crate::app) connected_session_id: Option<String>,
    pub(in crate::app) connected_session_state: Option<mbv_core::api::SessionInfo>,
    /// Cast attachment, beside `connected_session_id`/`connected_session_state`
    /// above: `None` means no cast target is attached. See `cast_actions.rs`
    /// for attach/detach and `cast_status_actions.rs` for status polling.
    pub(in crate::app) cast_attachment: Option<CastAttachment>,
    pub(in crate::app) last_cast_poll: Instant,
    pub(in crate::app) cast_status_loading: bool,
    pub(in crate::app) queue_epoch: QueueEpoch,
    pub(in crate::app) playlist_mutations: std::collections::HashMap<String, PlaylistMutationState>,
    pub(in crate::app) next_playlist_mutation: u64,
    pub(in crate::app) next_owner_queue_load_request: u64,
    pub(in crate::app) remote: crate::app::state::remote_tracking::RemoteTracking,
    pub(in crate::app) suspended_local: Option<SuspendedLocalSession>,
    /// The library route currently driving playback, if any (#223):
    /// `Some(name)` holds the lowercased library name whose configured
    /// daemon is the active player target. `None` means local playback,
    /// or a Sessions-panel direct remote (`connected_session_id` /
    /// `direct_remote_label`) -- a separate concept, never conflated with
    /// this one. Fixed for the life of the current queue: a *new* queue
    /// re-evaluates it (see `apply_route_for_playback`), but enqueuing
    /// into the existing queue must match it or be rejected (see
    /// `enqueue_route_conflict`).
    pub(in crate::app) active_route: Option<String>,
    /// Per-item cache of ancestor-lookup library-route resolution for
    /// cross-library aggregate views (Continue Watching/Next Up,
    /// Favorites), keyed by item id. `Some(name)` = resolved to that
    /// library (lowercased); `None` = resolved, no owning library route.
    /// Avoids a repeat `get_ancestors` round-trip for the same item
    /// within a session (#223). Each entry also carries the `Instant` it
    /// was cached at, so a mid-session library reorganization on the
    /// Emby server self-heals after `LIBRARY_ROUTE_CACHE_TTL` instead of
    /// requiring an app restart (#223, post-grilling revision item 5).
    pub(in crate::app) library_route_cache:
        std::collections::HashMap<String, (Option<String>, Instant)>,
    /// Whether prefix mode is armed (change `add-configurable-keybinds`,
    /// design D6, task 6.1): the App-owned bit the keyboard policy sees as
    /// `RouterSnapshot.prefix_armed`. Armed by the `prefix_arm` policy layer,
    /// disarmed by an armed-dispatch outcome, and silently cleared on any
    /// mouse event. Sticky until a disarm event; no timed expiry.
    pub(in crate::app) prefix_armed: bool,
    pub(in crate::app) force_clear: bool,
    pub(in crate::app) tab_scroll: usize,
    pub(in crate::app) ui_volume: u8,
    pub(in crate::app) pre_mute_volume: Option<u8>,
    pub(in crate::app) mute_on: bool,
    pub(in crate::app) visualizer_enabled: bool,
    pub(in crate::app) visualizer_failed: bool,
    pub(in crate::app) visualizer: Option<PipeWireWorker>,
    pub(in crate::app) visualizer_window: StereoSampleWindow,
    pub(in crate::app) visualizer_glyph: String,
    /// Text and start time of the shared marquee clock, reset whenever the
    /// tracked text changes so a new string always starts its scroll from
    /// the beginning rather than mid-cycle. Used by all marquee callers
    /// (mini-view "On Now", standard title row, idle feed title).
    pub(in crate::app) last_nav_at: Instant,
    pub(in crate::app) last_library_nav_at: Instant,
    /// Tracks terminal focus and arms a grace window to swallow the
    /// click that merely brings the window into focus.
    ///
    /// State transitions:
    /// - `None`: terminal is unfocused (or never reported focus).
    ///   All mouse button/scroll events are suppressed.
    /// - `Some(Instant)`: terminal is focused; the Instant records
    ///   when `FocusGained` was seen.  A click within `REFOCUS_WINDOW`
    ///   of that Instant is the refocusing click and is suppressed;
    ///   after the window expires the field stays `Some` so
    ///   subsequent clicks dispatch normally until the next
    ///   `FocusLost`.
    pub(in crate::app) refocus_at: Option<Instant>,
    pub(in crate::app) album_artist_cache: std::collections::HashMap<String, String>,
    /// Per-level album-artist fill lifecycle (design D4 of
    /// `fix-music-artist-resolution-batching`): one background request fills
    /// every album bucket in a level, so the fill state is keyed by level id
    /// rather than album id. Concurrent candidate creations and startup
    /// warm-up dedupe on this state.
    pub(in crate::app) album_artist_levels: std::collections::HashMap<String, LevelFillState>,
    /// Group-level artist fills waiting for one of the bounded warm-up slots.
    pub(in crate::app) pending_level_artist_warmups: std::collections::VecDeque<String>,
    /// Group-level artist fills currently occupying warm-up slots. Candidate
    /// requests share the level state but do not consume these slots.
    pub(in crate::app) level_artist_warmups_in_flight: std::collections::HashSet<String>,
    /// Track lists for the album currently highlighted in the
    /// album-folder listing, fetched proactively so the inline album detail
    /// pane (#145) has data without requiring the user to drill in first.
    /// Keyed by album id, mirroring `album_artist_cache`'s never-evicted
    /// lifetime.
    pub(in crate::app) album_tracks_cache: std::collections::HashMap<String, Vec<EmbyItem>>,
    pub(in crate::app) album_tracks_loading: std::collections::HashSet<String>,
    /// Fallback artist per-album track fetches waiting for a bounded slot
    /// (design D7, task 6.3). A fallback root's scope can be the whole
    /// settled catalog, so `enqueue_artist_album_tracks` arms at most
    /// `MAX_ARTIST_FALLBACK_TRACK_FETCHES` at a time and each arrival frees
    /// a slot for the next album.
    pub(in crate::app) pending_artist_album_track_fetches: std::collections::VecDeque<String>,
    /// Fallback artist per-album fetches currently occupying a bounded slot.
    /// Selection-driven album fetches share `album_tracks_cache` but not
    /// these slots.
    pub(in crate::app) artist_album_track_fetches_in_flight: std::collections::HashSet<String>,
    /// Artist detail tracks keyed by destination, Service setup generation,
    /// stable `ArtistItems` identity, and settled catalog revision (design
    /// D7, tasks 6.1–6.3). Shell-owned; the component never sees the cache.
    pub(in crate::app) artist_detail_cache: std::collections::HashMap<
        crate::app::state::music_artist_detail::ArtistDetailKey,
        crate::app::state::music_artist_detail::ArtistDetailCacheEntry,
    >,
    pub(in crate::app) artist_detail_loading:
        std::collections::HashSet<crate::app::state::music_artist_detail::ArtistDetailKey>,
    /// Typed identity hand-off for artist artwork fetched by the shared image
    /// worker. The image cache itself remains provider/generic.
    pub(in crate::app) artist_artwork_requests:
        std::collections::HashMap<String, crate::app::state::music_artist_detail::ArtistDetailKey>,
    pub(in crate::app) artist_artwork_status: std::collections::HashMap<
        crate::app::state::music_artist_detail::ArtistDetailKey,
        crate::app::state::music_artist_detail::ArtistArtworkStatus,
    >,
    /// TV series detail cache for inline rendering.
    /// When a Series is selected, we proactively fetch seasons and episodes
    /// so the inline detail pane can render without drilling in.
    pub(in crate::app) series_detail_cache: std::collections::HashMap<String, SeriesDetail>,
    pub(in crate::app) series_detail_loading: std::collections::HashSet<String>,
    pub(in crate::app) series_season_loading: std::collections::HashSet<(String, String)>,
    /// Season expansions received before their show's detail has arrived.
    pub(in crate::app) pending_series_season_expansions:
        std::collections::HashSet<(String, String)>,
    pub(in crate::app) images: crate::app::infra::images::cache::ImageCache,
    pub(in crate::app) library_position_state: crate::config::LibraryPositionState,
    pub(in crate::app) queue_scope: QueueScope,
    pub(in crate::app) idle_feed: Option<IdleFeed>,
    pub(in crate::app) feed_tab: FeedTabState,
    /// Local, machine-scoped feed-entry playback state (resume position and
    /// watched flag) for every configured subscription's entries. Loaded once
    /// at startup; rewritten on each playback lifecycle write. Replaces the
    pub(in crate::app) feed_entry_state: mbv_core::feed_entry_state::FeedEntryStore,
    /// When a seek was issued during Feed playback, the `slot_id` is stored
    /// here. The next `OutputStarted` clears it and persists the resulting
    /// position. This prevents ordinary output restarts (buffering,
    /// startup) from becoming state writes.
    pub(in crate::app) feed_seek_pending_slot: Option<mbv_core::playback_queue::QueueSlotId>,
    #[cfg(test)]
    pub(in crate::app) _test_state_dir_guard: Option<crate::config::TestStateDirGuard>,
}

impl App {
    pub(in crate::app) fn request_sidebar_open(&mut self, sidebar: SidebarId) {
        self.pending_overlay =
            Some(crate::app::state::types::overlay::OverlayRequest::OpenSidebar(sidebar));
    }

    pub(in crate::app) fn request_sidebar_dismiss(&mut self, sidebar: SidebarId) {
        self.pending_overlay =
            Some(crate::app::state::types::overlay::OverlayRequest::DismissSidebar(sidebar));
    }

    pub(in crate::app) fn request_sidebar_toggle(&mut self, sidebar: SidebarId) {
        self.pending_overlay =
            Some(crate::app::state::types::overlay::OverlayRequest::ToggleSidebar(sidebar));
    }

    pub(in crate::app) fn ask_confirm(&mut self, modal: ConfirmModal) {
        self.pending_overlay = Some(crate::app::state::types::overlay::OverlayRequest::Confirm(
            modal,
        ));
    }

    pub(in crate::app) fn open_save_playlist_dialog(&mut self, dialog: SavePlaylistDialog) {
        self.pending_overlay =
            Some(crate::app::state::types::overlay::OverlayRequest::SavePlaylist(dialog));
    }

    pub(in crate::app) fn dismiss_confirm(&mut self) {
        self.pending_overlay =
            Some(crate::app::state::types::overlay::OverlayRequest::DismissConfirm);
    }

    pub(in crate::app) fn dismiss_daemon_lost(&mut self) {
        self.pending_overlay =
            Some(crate::app::state::types::overlay::OverlayRequest::DismissDaemonLost);
    }

    pub(in crate::app) fn dismiss_save_playlist(&mut self) {
        self.pending_overlay =
            Some(crate::app::state::types::overlay::OverlayRequest::DismissSavePlaylist);
    }
}
