use super::panel_targets::PanelTarget;
use crate::app::QueueDeferrals;
use crate::app::state::client_capabilities::ClientCapabilities;
use crate::app::state::events::{PendingSeriesHandoff, PendingSeriesLanding};
use crate::app::state::playback::{
    DestinationLatestSource, HomeContent, PlaylistMutationState, SuspendedLocalSession, UndoEntry,
};
use crate::app::state::queue_owner::QueueEpoch;
use crate::app::state::queue_view::QueueView;
use crate::app::state::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use indexmap::IndexMap;
use mbv_ctrl::player::PlayerEvent;
use mbv_emby_model::EmbyItem;
use mbv_player::PlayerProxy;
use mbv_queue::QueueSlotId;
use mbv_ui_model::browse::{AlbumIndexState, SeriesDetail};
use mbv_ui_model::feed::IdleFeed;
use mbv_ui_model::feed_tab::FeedTabState;
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::playback::QueueLink;
use mbv_ui_model::settings::{PanelFocus, PanelMode, SettingsDestination};
use mbv_ui_model::tab_selection::TabSelection;
use mbv_visualizer::{PipeWireWorker, StereoSampleWindow};
use mbv_ws::WsEvent;
use std::sync::mpsc;
use std::time::Instant;

mod fill_state;
mod home_content;
mod home_recently_added;
mod launch_restore;
mod waker;

pub(in crate::app) use self::fill_state::{
    LevelFillAction, LevelFillState, MAX_ALBUM_TRACKS_CACHE, MAX_SERIES_DETAIL_CACHE,
    TvUpcomingFetchState, insert_capped,
};
pub(in crate::app) use self::launch_restore::LaunchRestore;

#[expect(
    clippy::struct_excessive_bools,
    reason = "22 independent core app-state bits spanning launch/readiness, load-in-flight, persistence/UI options, and transient connection flags across unrelated subsystems; grouping would be a pure lint dodge (design analysis, issue #804)"
)]
pub struct App {
    /// General application configuration is independent of the optional Emby
    /// runtime. Feed management reads and mutates this context directly.
    pub(in crate::app) config: std::sync::Arc<std::sync::Mutex<crate::app::config::Config>>,
    pub(in crate::app) emby_runtime: EmbyRuntime,
    pub(in crate::app) audiobookshelf_runtime: AudiobookshelfRuntime,
    pub(in crate::app) setup: crate::app::state::service_setup::ServiceSetup,
    pub(in crate::app) audiobookshelf_libraries: Vec<mbv_audiobookshelf::AudiobookshelfLibrary>,
    /// Most-recent `Newest Episodes` shelf per podcast library (async shelf
    /// fetch), keyed by library id. `fetch_home()` rebuilds Home's
    /// Audiobookshelf Latest pills from this cache — never a blocking network
    /// call — and the shelf-fetch handler refreshes it.
    pub(in crate::app) audiobookshelf_shelf_cache:
        std::collections::HashMap<String, Vec<mbv_queue::AudiobookshelfEpisodeCatalog>>,
    /// Podcast libraries whose default-Latest shelf fetch was already
    /// spawned, so repeated content pushes never re-request it.
    pub(in crate::app) audiobookshelf_shelf_requested: std::collections::HashSet<String>,
    /// Per-session TV Upcoming rows per library view (user rule, 2026-10-11):
    /// the eager fetch runs once when the TV tab loads, pill selection
    /// materializes the root level from here instead of refetching, and only
    /// the focused manual refresh replaces the snapshot.
    pub(in crate::app) tv_upcoming_snapshots:
        std::collections::HashMap<String, Vec<mbv_emby_model::UpcomingPremiere>>,
    /// TV libraries whose eager Upcoming fetch was already requested this
    /// session, so tab re-entry never re-requests it. `InFlight` spans the
    /// fetch's spaced retries; only a final failure moves the entry to
    /// `Failed`, which a later pill selection or tab load retries.
    pub(in crate::app) tv_upcoming_fetches: std::collections::HashMap<String, TvUpcomingFetchState>,
    pub(in crate::app) audiobookshelf_browse:
        Vec<mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState>,
    pub(in crate::app) audiobookshelf_book_browse:
        Vec<mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState>,
    pub(in crate::app) player: PlayerProxy,
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
    pub(in crate::app) mpris: Option<mbv_desktop::mpris::MprisHandle>,
    /// The shared wake handle (design D7 of `extract-client-core`): the
    /// Owner-snapshot link relays clone it and call `wake` after each
    /// forwarded event. `ClientCore` shares this handle, so a Client's
    /// installed waker reaches every relay — including one the initial
    /// attach already spawned.
    pub(in crate::app) client_waker: crate::app::ClientWaker,
    /// The channel the GUI Library's Client-only fetch workers send on (design
    /// D5 of `decouple-client-core`): the workers clone the sender, and the
    /// GUI Client's core takes the receiver once it exists. The TUI Client
    /// never starts one of these fetches, so it never receives an event.
    pub(in crate::app) library_fetch_events:
        crate::app::state::library_fetch_events::LibraryFetchEvents,
    /// The Owner program this Client starts (design D9 of
    /// `extract-client-core`). `None` starts
    /// [`crate::app::owner_link::owner_program`]: the `mbv-owner` program
    /// beside the running Client, else `mbv-owner` on `PATH`, so the Owner is
    /// never the running Client's own program. The GUI Client records the
    /// program it resolved, so an Owner restart never starts the GUI program
    /// as the Owner.
    pub(in crate::app) owner_exe: Option<std::path::PathBuf>,
    pub(in crate::app) player_rx: mpsc::Receiver<PlayerEvent>,
    /// Player events the answer pump set aside while waiting for a queue-op
    /// answer. `drain_player_events` takes these before
    /// `player_rx`, so deferred disconnects, restarts, and projections keep
    /// running at the tick level, in order.
    pub(in crate::app) deferred_player_events: std::collections::VecDeque<PlayerEvent>,
    /// Player events deferred from the suspended home link during a Local
    /// queue-op answer pump; only the home-link drain may interpret them.
    pub(in crate::app) deferred_home_events: std::collections::VecDeque<PlayerEvent>,
    pub(in crate::app) ws_rx: mpsc::Receiver<WsEvent>,
    pub(in crate::app) transport_rx: mpsc::Receiver<mbv_ctrl::TransportCommand>,
    pub(in crate::app) audiobookshelf_socket_rx:
        mpsc::Receiver<mbv_audiobookshelf::socket::SocketEvent>,
    pub(in crate::app) audiobookshelf_socket_tx: Option<mpsc::Sender<()>>,
    pub(in crate::app) audiobookshelf_socket_generation:
        Option<mbv_core::service_runtime::SetupGeneration>,
    pub(in crate::app) libs: Vec<LibraryTab>,
    pub(in crate::app) local_view: QueueView,
    pub(in crate::app) remote_view: Option<QueueView>,
    /// The local Owner's Named queues as the last `NamedQueueList` reported
    /// them (gui-named-queues design D7). Empty unless this Client negotiated
    /// `named-queues`, which only a GUI Client's local link does.
    pub(in crate::app) named_queues: crate::app::state::named_queues::NamedQueueList,
    /// The GUI Client's shown entry: the local Named queue or the connected
    /// peer whose queue the Queue panel, the player panel, and the edits follow
    /// (design D7, ADR 0041). `None` for the TUI Client, which keeps the
    /// connection rule (ADR 0034).
    pub(in crate::app) shown_entry: Option<crate::app::state::playback::ShownEntry>,
    /// The shown resting queue's own view, beside the current queue's
    /// `local_view` (design D7): filled from `NamedQueueSnapshot` while the GUI
    /// Client shows a queue that rests.
    pub(in crate::app) shown_resting: Option<(mbv_queue::NamedQueueId, QueueView)>,
    /// The `op` of the new-queue request the control sent and the Owner has
    /// not answered yet (gui-named-queues design D8). The Owner answers it with
    /// the new queue's own snapshot under this `op`, so the answer names the
    /// queue that the control then shows with its name open for edit. `None`
    /// while no request is in flight.
    pub(in crate::app) pending_new_queue: Option<mbv_ctrl::QueueOpId>,
    /// The `op` of every Named-queue command whose answer the Client does not
    /// pump for — the close and the rename the Queue chooser sends, beside the
    /// create the control holds in `pending_new_queue` (gui-named-queues design
    /// D8). Each answer arrives in a drain, so this set is how a rejection
    /// finds the request it answers and tells the user why; an op leaves the
    /// set when its answer arrives, and a lost Owner clears it.
    pub(in crate::app) pending_named_queue_ops: std::collections::HashSet<mbv_ctrl::QueueOpId>,
    /// The queue a new-queue request created and the text its rename draft
    /// opens with, handed to the GUI Client's core on its next input pass
    /// (gui-named-queues design D8). The core makes the queue, so it sets this;
    /// the overlay and the cursor that names its entry are GUI presentation, so
    /// the GUI core takes it and a Client with no chooser leaves it unset.
    pub(in crate::app) pending_queue_rename: Option<(mbv_queue::NamedQueueId, String)>,
    /// The Client core's live toasts (design D1): one list in one place, so
    /// several toasts are live at once, with the cap, the repeat rule, and
    /// the per-severity expiry owned by `ToastList`. The status row presents
    /// the newest toast that renders in-app; the GUI Client paints a card per
    /// toast.
    pub(in crate::app) toasts: crate::app::state::toasts::ToastList,
    /// The daemon endpoint for the current player target. `None` means an
    /// in-process player (bare mode). `Some(DaemonEndpoint::Local)` is this
    /// machine's managed local daemon. `Some(Tcp | Unix)` is a different
    /// daemon. Replaces the mutable `is_local_daemon` boolean so every
    /// transition records its source of truth rather than projecting it
    /// down to a bool that must be manually kept in sync.
    pub(in crate::app) player_endpoint: Option<mbv_remote_player::DaemonEndpoint>,
    pub(in crate::app) hidden_libraries: Vec<String>,
    /// Home content (add-gui-library-panel row 6.1): the authoritative
    /// Continue Watching snapshot both Clients store. The launch Home load
    /// fills it (`App::load_home_at_startup`), a rebuilt startup-drain
    /// snapshot replaces it (`App::drain_startup_workers`), and App-internal
    /// refresh writers hand their snapshot to the shell, whose assignment
    /// lands here. The shell reads it for its Home paint; the GUI Library's
    /// later Home projection reads it from row 6.2.
    pub(in crate::app) home_content: HomeContent,
    /// Home's per-source Recently added rows (add-gui-library-panel row 6.2;
    /// spec `home-latest-sections`, "Recently added combines every
    /// Latest-bearing destination"): each Service's fetch items, already
    /// filtered to the 14-day cutoff at store time and keyed by source. The
    /// accepted fetch event fills it (`App::handle_lib_event`'s arm), a
    /// rebuilt or cleared setup evicts its sources, and the Home projections
    /// — the shell's pushes and the GUI Library's later one — merge it with
    /// the Feeds entries (`App::home_recent_rows`). Feeds has no entry; its
    /// rows derive at merge time.
    pub(in crate::app) home_recently_added:
        std::collections::HashMap<DestinationLatestSource, Vec<mbv_queue::QueueItem>>,
    /// `Config.library_routes` at startup (#256). Values are resolved
    /// `tcp://host:port` endpoints, read directly with no live-session
    /// lookup -- see `mbv_remote_player::resolve_library_route`.
    pub(in crate::app) library_routes: std::collections::BTreeMap<String, String>,
    pub(in crate::app) music_levels: Vec<String>,
    pub(in crate::app) album_indexes: std::collections::HashMap<String, AlbumIndexState>,
    /// The left panel column of the last composed frame, as a neutral
    /// rectangle (design D13 Rule B). The TUI shell owns its layout record;
    /// the core keeps only this area, which the TUI pushes from the frame it
    /// composed (`App::set_left_panel_area`). A Client that composes no such
    /// frame leaves it zero, which is what the shell's default layout gave
    /// the core readers before the move.
    pub(in crate::app) left_panel_area: mbv_ui_model::geom::Rect,
    /// The single-panel fact the TUI pushes from its terminal width
    /// (`App::set_single_panel`). `is_mini_view` reads it.
    pub(in crate::app) single_panel: bool,
    /// Shell handoff for a modal raised by App-owned effects. The mounted
    /// component owns the modal after the next Model tick.
    pub(in crate::app) pending_overlay: Option<mbv_ui_model::overlay::OverlayRequest>,
    /// Set right before requesting a clean exit on an announced daemon
    /// shutdown; printed once by `run` after the terminal is
    /// restored, since anything written while still in the alternate screen
    /// would never be visible. `None` on every other exit path.
    pub(in crate::app) pending_exit_message: Option<String>,
    pub(in crate::app) pending_delete_slot: Option<QueueSlotId>, // marks a delete that was already applied optimistically, so the Stopped handler doesn't re-derive it
    /// A confirmed multi-select removal that included the now-playing row:
    /// every named slot is removed in one owner edit from the `Stopped`
    /// event that follows the confirming stop. `None` when no such
    /// multi-delete is awaiting its stop.
    pub(in crate::app) pending_delete_selection: Option<Vec<QueueSlotId>>,
    pub(in crate::app) queue_undo_stack: Vec<UndoEntry>,
    pub(in crate::app) remote_queue_undo_stack: Vec<UndoEntry>,
    /// One-shot cursor re-anchor for the next queue sync.
    pub(in crate::app) pending_queue_cursor_reanchor: Option<QueueLink>,
    /// One-shot playback-target refresh for the next tick: a Client whose own
    /// startup step begins the discovery asks for it while no Emby client
    /// exists yet, and the tick retries it once a client does. Cleared when
    /// the discovery starts.
    pub(in crate::app) pending_playback_target_refresh: bool,
    /// One-shot presentation reset for the next queue sync: the shown entry
    /// changed, so the Queue Surface drops its marks and viewport before it
    /// adopts the start-slot cursor (design D7 of `gui-named-queues`).
    pub(in crate::app) pending_queue_surface_reset: bool,
    /// Fingerprint of the queue projection `queue_surface_projection` last
    /// pushed to a Client's Queue Surface state. The projection runs once per
    /// Client tick; rebuilding the row vec (slot clone + per-row `format!`)
    /// on a tick where nothing the projection depends on changed is pure
    /// waste (#675). The fingerprint gates the rebuild: queue revision +
    /// viewed link + active slot + a progress-% bucket + the pending target.
    pub(in crate::app) last_queue_projection:
        Option<crate::app::queue_surface::QueueProjectionFingerprint>,
    /// The at-most-one in-flight queue transfer (queue-handoff-and-take
    /// design D2): armed after the replacement is sent to the target and
    /// settled in the tick phase that adopts owner snapshots.
    pub(in crate::app) pending_queue_transfer:
        Option<crate::app::state::queue_transfer::PendingQueueTransfer>,
    /// The queue transfer staged for its confirmation prompt
    /// (queue-handoff-and-take design D1): the resolved target plus, when the
    /// target link was not the current attachment, the link opened to read
    /// the target's queue. Confirm attaches that link; cancel drops it.
    /// `None` when nothing is staged.
    pub(in crate::app) staged_queue_transfer:
        Option<crate::app::state::queue_transfer::StagedQueueTransfer>,
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
    /// The neutral facts this Client differs on (design D6a): each capability
    /// stands for one thing a Client presents, so the core reads a fact and
    /// never names a Client. Each Client builds its own value: `App::build`
    /// starts empty and the Owner attach records the Client's own set before
    /// its first step. The terminal-panel rules and the visual-slot controls
    /// gate the mini view, the hidden empty queue column, and the visualizer
    /// and visual-slot hide setters.
    pub(in crate::app) client_capabilities: ClientCapabilities,
    /// The one startup snapshot and its tab-scoped destination re-anchor.
    pub(in crate::app) launch_restore: LaunchRestore,
    /// Deferred tab switch for a `NavigateLanding::Album` landing: set when the
    /// recursive
    /// album activation spawns, consumed on its `RecursiveAlbumActivated`
    /// drain once the landed nav stack has replaced the saved position, so
    /// the switch's activation never restores a stale position. Consumed only
    /// when the drained activation belongs to the pending library; any other
    /// library's activation leaves it armed. A manual tab change
    /// (`apply_tab_position`), `LibEvent::Error`, or
    /// `LibEvent::FeedHomeVideoSelectionFailed` clears it, so a user who moved
    /// on or a failed activation never gets yanked (U2 correction).
    pub(in crate::app) pending_navigate_tab_switch: Option<usize>,
    /// Ensure-then-land state for a `NavigateLanding::Series` whose target
    /// library corpus is not ready (U2 correction): armed by the
    /// `NavigateTo` arm, retried on that library's `Loaded`,
    /// `AllItemsPrefetched` and restored-position drains. See
    /// `PendingSeriesLanding` for the full lifecycle.
    pub(in crate::app) pending_series_landing: Option<PendingSeriesLanding>,
    /// A landing that completed and still owes the shell's Series detail
    /// hand-off: armed by the landing success path and
    /// consumed by the shell's sync pass, which runs the Inline Search series
    /// presentation sequence. The deferred `PendingSeriesLanding` retry arms
    /// it the same way, so the hand-off fires at the actual landing
    /// completion, not at the original `NavigateTo` drain.
    pub(in crate::app) pending_series_handoff: Option<PendingSeriesHandoff>,
    /// Deep selection: the chosen track from a
    /// `NavigateLanding::Album`, armed when the recursive album activation
    /// spawns and bound to the activated album at the shell's
    /// `RecursiveAlbumActivated` drain. Keyed by the target library so a
    /// foreign library's activation never adopts it.
    pub(in crate::app) pending_track_selection: Option<(usize, String)>,
    /// The queue visual slot's image projection: the queue
    /// projection issues every fetch for the now-playing item and projects
    /// the slot's image state; the painter reads this and paints. Refreshed
    /// by `Model::sync_queue`'s push while playback is active.
    pub(in crate::app) queue_card_projection: mbv_ui_model::playback::QueueCardProjection,
    pub(in crate::app) title_log_gate: crate::app::state::projection::card::artwork::TitleLogGate,
    pub(in crate::app) dim_backdrop_active: bool,
    /// Whether the terminal paints halfblocks rather than a pixel protocol:
    /// a pushed fact (change `split-tui-gui-clients`, design D2 rule 3) that
    /// the TUI shell derives from its image pickers (`App::set_halfblock_facts`).
    /// The queue title site reads it, since a halfblock card cannot carry the
    /// title in its artwork. The constructor's default is false — until a
    /// Client pushes the facts its pickers are unknown, and the site keeps the
    /// title on the artwork, as the pre-move check did for a Client that
    /// initializes no pickers.
    pub(in crate::app) halfblock_configured: bool,
    /// Whether this Client's image pickers are initialized at all. The same
    /// pushed fact record as `halfblock_configured`.
    pub(in crate::app) image_pickers_ready: bool,
    /// The configured picker's terminal font size — the cell-to-pixel ratio
    /// the queue card's title overlay is composed at. The same pushed fact
    /// record; the constructor's default until the pickers initialize.
    pub(in crate::app) image_font_size: (u16, u16),
    /// The picker that paints under the artwork cache key's current suffix —
    /// the halfblocks fallback's cell while a dimming backdrop forces it
    /// (#451), else the configured picker's — in pixels. The hero cover fit's
    /// box pixels derive from it, so a paint under the fallback crops on the
    /// grid it actually paints. The same pushed fact record, re-pushed when
    /// the dim state changes.
    pub(in crate::app) active_image_font_size: (u16, u16),
    pub(in crate::app) settings_destination: SettingsDestination,
    pub(in crate::app) settings_save_at: Option<Instant>,
    /// The Settings feeds-management popup's background add-feed channel,
    /// its in-flight add marker, and its attempt-id counter
    /// (add-gui-settings-screen design D7): the piece of the Feeds-manage
    /// overlay that cannot live in the interaction component, so both
    /// Clients share it. `Some` only while a feed-manage surface is open;
    /// the mpsc receiver drops on close, discarding any in-flight result.
    pub(in crate::app) feeds_manage: Option<mbv_ui_model::feeds_manage::FeedsManagePopup>,
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
    /// overlay is active.
    pub(in crate::app) sessions: Vec<mbv_emby::SessionInfo>,
    /// Last cast discovery browse result (8.1), independent of `sessions`'s
    /// own reload cadence -- see `panel_targets::build_panel_targets`.
    pub(in crate::app) cast_receivers: Vec<mbv_cast::discovery::CastReceiver>,
    /// The F3 panel's merged Emby+Cast target list, rebuilt from `sessions`/
    /// `cast_receivers` by `App::rebuild_panel_targets` (8.1/8.2).
    pub(in crate::app) panel_targets: Vec<PanelTarget>,
    pub(in crate::app) sessions_loading: bool,
    /// Whether a cast discovery browse (`spawn_cast_discovery`) is in flight.
    /// Set when one starts and cleared when its completion is handled, so the
    /// playback-target refresh does not stack a second browse on the first.
    pub(in crate::app) cast_discovery_loading: bool,
    pub(in crate::app) playlists: Vec<EmbyItem>,
    pub(in crate::app) playlists_loading: bool,
    pub(in crate::app) playlists_open: Option<EmbyItem>, // playlist currently being browsed
    pub(in crate::app) playlists_open_items: Vec<EmbyItem>,
    pub(in crate::app) playlists_open_loading: bool,
    pub(in crate::app) queue_dirty: bool,
    pub(in crate::app) pending_owner_source_update:
        Option<(mbv_queue::QueueSource, mbv_queue::QueueLineage)>,
    /// All deferred queue payloads and their ownership transitions live in
    /// `state::queue_deferrals`; this is the only place slot ownership lives.
    pub(in crate::app) queue_deferrals: QueueDeferrals,
    pub(in crate::app) use_nerd_fonts: bool,
    pub(in crate::app) indicator_style: mbv_ui_model::indicators::IndicatorStyle,
    pub(in crate::app) ws_send_tx: Option<mbv_ws::WsSender>,
    pub(in crate::app) last_keepalive: Instant,
    pub(in crate::app) last_capabilities: Instant,
    /// The current Player owner's active playback output, mirrored from the
    /// last `CtrlEvent::PlaybackOutput`. It is
    /// per-owner-connection state like `ctrl_compatibility`: `None` until the
    /// owner reports, reset to `None` when the connection is replaced by
    /// another owner or observed lost. Every owner built with
    /// cast-owner-output cold-seeds it right after connect, so the `None`
    /// window after a swap is one event beat. `None` reads as Local: an
    /// owner that never reports (pre-change peer) has no Cast output.
    ///
    /// This mirror is drain-fed — it only moves when the shell drains
    /// player events — so it is presentation truth, not control truth.
    /// Launch auto-reconnect connects a session (and severs the old target)
    /// before any drain ever happens, so control decisions that must not
    /// wait for a beat read the owner connection's retained state instead
    /// (`RemotePlayer::playback_output_state`, which the handshake fills at
    /// the cold seed) — see `hand_remote_output_back_to_local` (8.2 fix).
    pub(in crate::app) playback_output: Option<mbv_ctrl::OutputState>,
    pub(in crate::app) queue_epoch: QueueEpoch,
    pub(in crate::app) playlist_mutations: std::collections::HashMap<String, PlaylistMutationState>,
    pub(in crate::app) next_playlist_mutation: u64,
    /// Direct remote control target state. The owner holds any Emby Session
    /// output; only the Direct remote control connection is tracked here.
    pub(in crate::app) direct_remote_connected: bool,
    pub(in crate::app) direct_remote_label: Option<String>,
    /// Emby session id of a remote owner under Direct remote control, so the
    /// Sessions sidebar can still mark that row after the control socket
    /// takes over. Never set for the Stay-alive process: that process is not
    /// a remote session.
    pub(in crate::app) direct_remote_session_id: Option<String>,
    pub(in crate::app) suspended_local: Option<SuspendedLocalSession>,
    /// The library route currently driving playback, if any (#223):
    /// `Some(name)` holds the lowercased library name whose configured
    /// daemon is the active player target. `None` means local playback,
    /// or a Sessions-panel direct remote (`direct_remote_label`) -- a
    /// separate concept, never conflated with this one. Fixed for the life
    /// of the current queue: a *new* queue re-evaluates it (see
    /// `apply_route_for_playback`), but enqueuing into the existing queue
    /// must match it or be rejected (see `enqueue_route_conflict`).
    pub(in crate::app) active_route: Option<String>,
    /// Whether this run itself left a restore-able remote target by an
    /// explicit user action (a Direct remote detach, or a confirmed local
    /// fall-through), and has not attached a new target since. Teardown
    /// clears the saved auto-reconnect record only when this is set: an
    /// exit that merely had no remote at the moment -- a Client that never
    /// touched the connection this run, a failed or never-attempted launch
    /// restore, a mid-run daemon death -- must not erase the record
    /// another Client saved.
    pub(in crate::app) left_remote_this_run: bool,
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
    /// Whether prefix mode is armed: the App-owned bit the keyboard policy sees
    /// as
    /// `RouterSnapshot.prefix_armed`. Armed by the `prefix_arm` policy layer,
    /// disarmed by an armed-dispatch outcome, and silently cleared on any
    /// mouse event. Sticky until a disarm event; no timed expiry.
    pub(in crate::app) prefix_armed: bool,
    /// The queue filter's text: it narrows the projected rows locally and
    /// never touches the canonical queue. Empty text filters nothing.
    pub(in crate::app) queue_filter: String,
    /// Whether the queue filter's text entry holds the keyboard: the fact
    /// that suppresses the global bindings while the user types.
    pub(in crate::app) queue_filter_active: bool,
    pub(in crate::app) force_clear: bool,
    /// Set by the core where the tab strip must reveal the current tab
    /// (design D6b): the tab-strip scroll is TUI presentation, so the TUI
    /// shell runs its reveal rule after every update step and tick drain and
    /// clears this. The GUI ignores it, since it has its own tab strip.
    pub(in crate::app) tab_reveal_requested: bool,
    /// Emby libraries whose Wide TV series Workspace the Client last painted
    /// wide. The breakpoint is terminal geometry, so only a Client computes
    /// it; this set is the Client's write-back, and the core reads it for the
    /// one activation branch both Clients reach (design D2). The TUI sets it
    /// from its painted layout each sync pass; a Client that paints no such
    /// workspace leaves it empty.
    pub(in crate::app) tv_workspace_wide: std::collections::HashSet<usize>,
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
    /// The instant of the most recent terminal focus report, kept alongside
    /// `window_focused` (`note_focus_gained`/`note_focus_lost`): `Some` from
    /// the last `FocusGained`, `None` after `FocusLost`. No production code
    /// reads it today; test stubs set it to a past instant to model an
    /// already-focused window.
    pub(in crate::app) refocus_at: Option<Instant>,
    /// Whether the terminal window currently holds focus, per the terminal
    /// focus reports (`note_focus_gained`/`note_focus_lost`). Defaults to
    /// `true`: a launch with no focus report means the window is focused,
    /// because pinwin reports only changes.
    pub(in crate::app) window_focused: bool,
    /// Album id to album artist, bulk-filled per level by the fetches that
    /// group views read. Uncapped on purpose: it holds one short row per album
    /// in the library, and warm-up fills every group level at startup, so the
    /// library is the bound. Rows are never evicted, because a `Filled` level
    /// never refetches; an evicted album would keep its fallback label. Cleared
    /// on Service swap.
    pub(in crate::app) album_artist_cache: std::collections::HashMap<String, String>,
    /// Normalized artist name -> Service artist item id, from the level fills'
    /// tracks (`ArtistItemIdsFetched`). Addresses a folder-based artist row's
    /// `Primary` image; never an artist key. Cleared on Service swap.
    pub(in crate::app) artist_name_item_ids: std::collections::HashMap<String, String>,
    /// Normalized artist name -> its artist search state while the name has no
    /// resolved id (`dispatch::library::artist_lookup`). Cleared on Service
    /// swap.
    pub(in crate::app) artist_name_lookups: std::collections::HashMap<
        String,
        crate::app::dispatch::library::artist_lookup::ArtistNameLookupEntry,
    >,
    /// Per-level album-artist fill lifecycle: one background request fills
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
    /// Keyed by album id. Bounded at `MAX_ALBUM_TRACKS_CACHE` (issue #917):
    /// insertions of fresh ids evict the oldest entry so a long music
    /// session cannot keep every fetched list reachable.
    pub(in crate::app) album_tracks_cache: IndexMap<String, Vec<EmbyItem>>,
    pub(in crate::app) album_tracks_loading: std::collections::HashSet<String>,
    /// Fallback artist per-album track fetches waiting for a bounded slot.
    /// A fallback root's scope can be the whole
    /// settled catalog, so `enqueue_artist_album_tracks` arms at most
    /// `MAX_ARTIST_FALLBACK_TRACK_FETCHES` at a time and each arrival frees
    /// a slot for the next album.
    pub(in crate::app) pending_artist_album_track_fetches: std::collections::VecDeque<String>,
    /// Fallback artist per-album fetches currently occupying a bounded slot.
    /// Selection-driven album fetches share `album_tracks_cache` but not
    /// these slots.
    pub(in crate::app) artist_album_track_fetches_in_flight: std::collections::HashSet<String>,
    /// Artist detail tracks keyed by destination, Service setup generation,
    /// stable `ArtistItems` identity, and settled catalog revision. Shell-owned;
    /// the component never sees the cache.
    pub(in crate::app) artist_detail_cache: std::collections::HashMap<
        mbv_ui_model::music_artist_detail::ArtistDetailKey,
        mbv_ui_model::music_artist_detail::ArtistDetailCacheEntry,
    >,
    pub(in crate::app) artist_detail_loading:
        std::collections::HashSet<mbv_ui_model::music_artist_detail::ArtistDetailKey>,
    /// Typed identity hand-off for artist artwork fetched by the shared image
    /// worker. The image cache itself remains provider/generic.
    pub(in crate::app) artist_artwork_requests:
        std::collections::HashMap<String, mbv_ui_model::music_artist_detail::ArtistDetailKey>,
    pub(in crate::app) artist_artwork_status: std::collections::HashMap<
        mbv_ui_model::music_artist_detail::ArtistDetailKey,
        mbv_ui_model::music_artist_detail::ArtistArtworkStatus,
    >,
    /// TV series detail cache for inline rendering.
    /// When a Series is selected, we proactively fetch seasons and episodes
    /// so the inline detail pane can render without drilling in.
    /// Bounded at `MAX_SERIES_DETAIL_CACHE` (issue #917): these are the
    /// heaviest cached payloads (a series' seasons plus each expanded
    /// season's full episode list), so insertions of fresh ids evict the
    /// oldest entry.
    pub(in crate::app) series_detail_cache: IndexMap<String, SeriesDetail>,
    pub(in crate::app) series_detail_loading: std::collections::HashSet<String>,
    /// Series whose own detail fetch runs, as distinct from the loading mark its
    /// season fetches also hold (unit 7 review, finding P1): a season landing
    /// keeps the derived mark while its series' detail fetch is in flight, and a
    /// stale detail waits for a running season fetch. Only the detail landing and
    /// the detail failure arm clear an id here.
    pub(in crate::app) series_detail_fetches_in_flight: std::collections::HashSet<String>,
    /// Series ids whose running detail fetch is a quiet in-view load (design
    /// D6): its landing caches the detail but starts no first-season episode
    /// fetch. A plain `fetch_series_detail` for the same id drops the mark.
    pub(in crate::app) series_detail_quiet: std::collections::HashSet<String>,
    /// Cached series whose detail an in-view refresh asked to reload (design
    /// D9): the landing replaces the cached detail instead of the first-wins
    /// rule, and a failed refetch clears the mark so the failure never retries
    /// on each landing.
    pub(in crate::app) series_detail_stale: std::collections::HashSet<String>,
    pub(in crate::app) series_season_loading: std::collections::HashSet<(String, String)>,
    /// Cached seasons whose episodes an in-view refresh asked to reload (design
    /// D9): the landing replaces the cached episodes, and a failed refetch
    /// clears the mark.
    pub(in crate::app) series_season_stale: std::collections::HashSet<(String, String)>,
    /// Season expansions received before their show's detail has arrived.
    pub(in crate::app) pending_series_season_expansions:
        std::collections::HashSet<(String, String)>,
    pub(in crate::app) images: mbv_images::cache::ImageCache,
    /// The suffix of the protocol the shell paints artwork through: the
    /// halfblocks fallback's while a dimming backdrop is up, else the
    /// configured one. The shell sets the configured suffix once at start-up
    /// (`App::set_configured_protocol_suffix`) because the cache keys carry
    /// it; the core never detects a protocol itself.
    pub(in crate::app) configured_protocol_suffix: &'static str,
    /// Cache keys a completed fetch stored for a Client that paints terminal
    /// protocols, oldest first: the shell's drain encodes each from the
    /// cached entry (`ImageProtocols` lives on the shell `Model`, design D2
    /// rule 3). Only keys travel, so the tick thread clones no bitmap.
    pub(in crate::app) pending_protocol_encodes: std::collections::VecDeque<String>,
    /// Cache keys whose stored protocol encoding must drop: the projection
    /// found the entry's pixels or its cover fit changed, so the next paint
    /// re-encodes from the retained source. The shell's drain clears them
    /// (design D2 rule 3).
    pub(in crate::app) pending_protocol_clears: Vec<String>,
    /// What this Client's image fetches keep (design D14), chosen at
    /// construction and passed to every fetch thread.
    pub(in crate::app) image_decode: mbv_images::ImageDecode,
    pub(in crate::app) library_position_state: mbv_queue::LibraryPositionState,
    pub(in crate::app) idle_feed: Option<IdleFeed>,
    pub(in crate::app) feed_tab: FeedTabState,
    /// Local, machine-scoped feed-entry playback state (resume position and
    /// watched flag) for every configured subscription's entries. Loaded once
    /// at startup; rewritten on each playback lifecycle write. Replaces the
    pub(in crate::app) feed_entry_state: mbv_feed::FeedEntryStore,
    /// When a seek was issued during Feed playback, the `slot_id` is stored
    /// here. The next `OutputStarted` clears it and persists the resulting
    /// position. This prevents ordinary output restarts (buffering,
    /// startup) from becoming state writes.
    pub(in crate::app) feed_seek_pending_slot: Option<mbv_queue::QueueSlotId>,
    /// The active width of the pinned panel; never persisted, so a launch
    /// always starts collapsed.
    /// Whether this launch is pinned at all comes from `pin::is_pinned()`,
    /// the panel identity claimed at start-up.
    pub(in crate::app) pinned_width: mbv_ui_model::settings::PinnedWidth,
    /// The one panel request whose `pinwin` child is still running. While it
    /// is set, `x` and F2 Panel steps are ignored (design D4/D6/D7).
    pub(in crate::app) pinned_request_pending:
        Option<crate::app::state::pinned_request::PinnedRequestPending>,
    /// The configured `Accent color` when it lay outside the row's fixed
    /// palette, held while Settings is open so cycling can return to it
    /// (issue #875). Cleared each time Settings mounts; never persisted.
    pub(in crate::app) accent_custom: Option<mbv_config::PanelAccentColor>,
    #[cfg(any(test, feature = "test"))]
    pub(in crate::app) _test_state_dir_guard: Option<crate::app::config::TestStateDirGuard>,
    /// How many cast discovery browses this `App` started. Only the Clients'
    /// tests read it: the GUI Client's startup regression asserts that the
    /// discovery began without waiting on a real browse.
    #[cfg(any(test, feature = "test"))]
    pub(in crate::app) cast_discovery_starts: usize,
}

impl App {
    /// The newest toast's text, whether or not its duration has run out, or
    /// `""` when there is none (design D5). A test read: the presented toasts
    /// come from `toast_presentation` and `toast_cards`.
    #[cfg(any(test, feature = "test"))]
    pub fn newest_toast_text(&self) -> &str {
        self.toasts.newest().map_or("", |toast| toast.text.as_str())
    }

    /// The newest toast's severity, whether or not its duration has run out,
    /// or the default when there is none (design D5). A test read.
    #[cfg(any(test, feature = "test"))]
    pub fn newest_toast_severity(&self) -> crate::app::dispatch::notify::ToastSeverity {
        self.toasts
            .newest()
            .map_or_else(Default::default, |toast| toast.severity)
    }

    /// A pinned launch's default screen is always the Queue panel, never the
    /// Library: it overrides both saved sources of Panel focus — the persisted
    /// `panel_focus` pref read at construct and the saved launch snapshot's
    /// `panel_focus` that `reanchor_pending_launch_destination` re-applies —
    /// so a Library-focused snapshot cannot land the pinned panel on the
    /// Library column. Runs before the restore settles, so only the
    /// `Pending` snapshot can exist here; the user can still switch focus
    /// once the session runs, and teardown keeps saving the actual focus.
    /// Adopt the GUI Client's launch-location rule (design D6a): the GUI's
    /// own launch location replaces the TUI's, so this `App` starts with
    /// `LaunchRestore::Done` and no TUI launch state moves its tab or Panel
    /// focus. A Client that arranges its own launch calls it before its first
    /// step; the TUI Client never does, so the TUI's launch restore does not
    /// change.
    pub fn skip_launch_restore(&mut self) {
        self.launch_restore = LaunchRestore::Done;
    }

    pub fn pin_launch_focus_to_queue(&mut self) {
        // Written directly, not through `set_panel_focus`: at construct time
        // the shell has not recorded a terminal size yet, and this rule must
        // not depend on which branch a size-dependent setter would take.
        self.panel_focus = PanelFocus::Queue;
        self.focus_queue_initial_item();
        if let LaunchRestore::Pending(state) = &mut self.launch_restore {
            state.panel_focus = mbv_config::LaunchPanelFocus::Queue;
        }
    }
}

/// `App` carries the shell's whole state machine, including live Player,
/// protocol, and channel handles, so its `Debug` names the type and stops
/// there instead of walking that state.
impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App").finish_non_exhaustive()
    }
}
