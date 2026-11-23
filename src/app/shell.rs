use mbv_theme as palette;
use std::time::Duration;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    AlbumIndexState, App, BrowseLevel, ConfirmAction, ConfirmModal, LibEvent, PanelFocus,
    PanelMode, QueueLink, SavePlaylistDialog, SavePlaylistStage, SidebarId, TabSelection,
    ToastSeverity,
};
use super::{QUIT_REQUESTED, install_signal_handlers, start_quit_watchdog};
pub(in crate::app) use crate::app::input::router::RouterOutcome;
use crate::app::state::playback::{DestinationLatestSnapshot, DestinationLatestSource};
use mbv_components::{QueueBoundaryComponent, UiRootComponent};
use mbv_ui_model::home_latest::HomeLatestLaunchWindow;
use mbv_ui_model::now_playing::WorkspaceNowPlaying;
use mbv_ui_msg::AlbumCursorKind;
#[cfg(test)]
pub(in crate::app) use mbv_ui_msg::OverlayId;
use mbv_ui_msg::{ComponentId, Msg, TerminalObserverEvent, UserEvent};
use tuirealm::application::{Application, PollStrategy};
use tuirealm::listener::EventListenerCfg;

mod arbitration;
#[cfg(test)]
pub(in crate::app) use arbitration::fold_keyboard_messages;
pub(in crate::app) type ArbitrationDiagnostic = arbitration::ArbitrationDiagnostic;
pub(in crate::app) use arbitration::{
    arbitrate_key, fold_mouse_messages, indexed, right_press_cell,
};
mod audiobookshelf_book;
mod audiobookshelf_podcast;
mod chrome_panels;
mod chrome_status;
pub(in crate::app) mod draw;
mod emby_library;
mod emby_library_content;
mod feeds;
mod feeds_manage;
mod focus;
mod hero_image_projection;
mod home;
mod home_content;
pub(in crate::app) mod image_protocol;
mod inline_search;
mod input_port;
mod library;
mod library_panel;
mod messages;
mod modal_actions;
mod music_workspace;
/// The TUI input edge: terminal keys and mouse events cross into the neutral
/// vocabulary here, before any core module reads them.
pub(in crate::app) mod neutral_edge;
pub(in crate::app) mod now_playing;
mod overlays;
mod playback;
mod playlists;
mod queue;
pub(in crate::app) mod queue_card;
mod root;
mod run;
mod settings;
mod tab_strip;
pub(in crate::app) mod teardown;
mod terminal;
pub(in crate::app) mod tv_wide;
pub(crate) use terminal::set_mouse_capture;
pub(in crate::app) use terminal::{init_terminal, restore_terminal};
mod tv_workspace;
mod twitch_url;
mod ui_state_reset;
mod visualizer;

/// How often the ``TuiRealm`` crossterm listener worker polls the terminal for
/// events. The listener's `poll` blocks for half of this; the worker cycle is
/// this long. Set to 8 ms so event latency matches the legacy loop's fastest
/// cadence (the visualizer's 8 ms poll). The main thread's per-iteration wait
/// is governed separately by the `PollStrategy::Once` timeout below, so this
/// only affects how promptly a buffered event reaches the channel — not the
/// render cadence.
const TERMINAL_LISTENER_INTERVAL: Duration = Duration::from_millis(8);
/// Upper bound on port polls, and so on events delivered to the channel, in one
/// worker cycle. The input port forwards at most one mouse move at a time, so a
/// burst of moves cannot flood the channel; the main thread still processes at
/// most one event per tick via `PollStrategy::Once`, matching the legacy
/// one-event-per-iteration loop.
const TERMINAL_LISTENER_MAX_POLL: usize = 60;

/// One-shot inline-track-focus transition the shell hands the Music workspace
/// at the next content push. `Enter` is bound to the album it was raised for,
/// so a re-anchor that outruns that album's track fetch can retry on the
/// tracks re-push without ever firing on a different album.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum MusicTrackFocusRequest {
    /// Enter inline track focus for this album (recursive album activation).
    Enter { album_id: String },
    /// Clear inline track focus (saved-position restore).
    Clear,
}

/// Deep selection (task 6.1, design D6 of change
/// `per-destination-item-navigation`): a navigated episode the opened TV
/// workspace still owes its selection. Consumed by
/// `drain_pending_episode_selection` once the series detail and the
/// episode's season episodes are in hand; absence of the episode from the
/// fetched detail clears it silently (the landing stands, no error).
#[derive(Clone, Debug)]
pub(in crate::app) struct PendingEpisodeSelection {
    pub(in crate::app) lib_idx: usize,
    pub(in crate::app) series_id: String,
    pub(in crate::app) episode_id: String,
}

/// Deep selection (task 6.2, design D6): a navigated track the adopted
/// Music workspace still owes its selection, bound to the activated album at
/// the `RecursiveAlbumActivated` drain. Consumed by
/// `push_music_workspace_content` once the album's track rows arrive;
/// absence clears it silently.
#[derive(Clone, Debug)]
pub(in crate::app) struct MusicTrackSelection {
    pub(in crate::app) album_id: String,
    pub(in crate::app) track_id: String,
}

/// A pending Music tree selection re-anchor. Every discrete navigation that
/// names a stable row (recursive-album activation, `NavigateTo` landing,
/// Inline Search activation) carries a [`MusicTreeTarget`]; a pill/group
/// switch replaces the album level wholesale, so its target is that new
/// level's first album, resolved once the level's content has landed.
///
/// [`MusicTreeTarget`]: mbv_components::music_tree_target::MusicTreeTarget
#[derive(Clone, Debug)]
pub(in crate::app) enum PendingMusicReanchor {
    Target(mbv_components::music_tree_target::MusicTreeTarget),
    FirstAlbumOfLevel,
}

/// Shell model holding the legacy `App` and the ``TuiRealm`` `Application`.
pub struct Model {
    pub app: App,
    pub(in crate::app) application: Application<ComponentId, Msg, UserEvent>,
    /// Components currently carrying the `mouse_sub()` subscription. Owned
    /// solely by `sync_mouse_subscriptions` (ADR 0024 D2): it is the mouse
    /// arbitration table. `tuirealm` 4.1's `Application::unsubscribe` removes
    /// every subscription matching the clause value at once (it cannot target
    /// one component's mouse sub in isolation), so the reconciler wipes and
    /// rebuilds the whole set on any change and this mirror is how it knows
    /// the current state without querying `Application`.
    pub(in crate::app) mouse_subscribed: std::collections::HashSet<ComponentId>,
    /// One-shot shell→component request for the mounted Music workspace's
    /// inline track focus, applied at the next `push_music_workspace_content`
    /// after the component is mounted/synced (so mount-timing never loses it).
    /// Neither mirrors App state: the component owns the cursor, the shell
    /// only delivers the trigger that used to write the deleted inline
    /// track-focus field.
    pub(in crate::app) music_track_focus_request: Option<MusicTrackFocusRequest>,
    /// Deep-selection pending state (tasks 6.1/6.2). TV: see
    /// `PendingEpisodeSelection`; consumed at the sync pass. Music: see
    /// `MusicTrackSelection`; consumed at the workspace content push.
    pub(in crate::app) pending_episode_selection: Option<PendingEpisodeSelection>,
    pub(in crate::app) pending_music_track_selection: Option<MusicTrackSelection>,
    /// One-shot shell→component selection re-anchor for the mounted Music
    /// owner, consumed at the next `push_music_workspace_content`. Set only
    /// at a discrete navigation event (recursive-album activation,
    /// `NavigateTo` landing, Inline Search activation, or a pill/group
    /// switch). Ordinary content pushes never re-point the tree: the mounted
    /// owner's local selection is authoritative across refreshes, tab
    /// changes, and async completions, and startup restores only through
    /// `reanchor_launch_state`.
    pub(in crate::app) pending_music_reanchor: Option<PendingMusicReanchor>,
    /// Shell-owned acknowledgement shared by Home and TV Latest surfaces.
    pub(in crate::app) acknowledged_home_latest_sources:
        std::collections::HashSet<DestinationLatestSource>,
    /// One authoritative TV Latest section snapshot per Emby library view.
    pub(in crate::app) tv_latest_snapshots:
        std::collections::HashMap<String, DestinationLatestSnapshot>,
    /// Emby libraries whose default-Latest fetch was already requested this
    /// run, so the every-sync-pass owner projection never re-requests it
    /// (mirrors `App.audiobookshelf_shelf_requested`; spec
    /// `destination-latest-modes`, "Destination Latest lists load lazily").
    pub(in crate::app) emby_destination_latest_requested: std::collections::HashSet<String>,
    pub(in crate::app) home_context_item: Option<mbv_emby_model::EmbyItem>,
    /// The last terminal size the sync pass applied resize side effects for
    /// (task 1.2). Initialized from the App's size so fixtures that pre-set a
    /// size never spuriously resize; the draw path's size normalization is
    /// picked up at the next sync pass.
    pub(in crate::app) handled_terminal_size: (u16, u16),
    /// Armed by the Resize observer (the real terminal-resize event) and
    /// consumed by the next sync pass, which then also applies the mini-view
    /// focus hand-off (task 1.2). Size normalization without a resize event
    /// (a direct frame, a fixture) never arms it.
    pub(in crate::app) pending_terminal_resize: bool,
    /// Terminal hyperlink support resolved once during terminal initialization.
    /// Shell-owned projection of the focused list's Visual selection.
    pub(in crate::app) visual_selection: Option<(mbv_ui_model::settings::PanelFocus, usize)>,
    pub(in crate::app) context_menu_origin: Option<mbv_ui_model::media_list::SelectionOrigin>,
    /// The cell the last right-click landed on, in terminal cells. The core
    /// builds every menu with the neutral `ContextMenuAnchor::Pointer` and no
    /// cell (design D6); the shell records the one `MouseRightClick` observer
    /// signal and places a pointer-anchored menu here. `None` leaves a pointer
    /// menu at its panel's top-left corner, as an unset anchor always did.
    pub(in crate::app) menu_pointer: Option<(u16, u16)>,
    /// Whether the message now being dispatched is the one this tick's
    /// right-click raised, so it anchors at `menu_pointer` instead of the
    /// core's selected-item anchor. A right-click and the component request it
    /// raises reach the shell in one tick, delivered in either order; the
    /// shell records the right-press position from the tick's raw messages
    /// (`Model::record_right_press`) and sets this per message as it dispatches
    /// them (`Model::scope_menu_pointer`), so a keyboard request arriving
    /// earlier in the same tick keeps the selected-item anchor.
    pub(in crate::app) menu_pointer_current: bool,
    /// The position in the tick's raw message list where the `MouseRightClick`
    /// observer report arrived; `None` for a tick without a right press. Menu
    /// requests at or after it are the ones this tick's right-click raised.
    pub(in crate::app) menu_pointer_press_at: Option<usize>,
    pub(in crate::app) context_action_snapshot: Option<
        mbv_ui_model::context_menu::ContextActionSnapshot<
            mbv_ui_model::context_menu::ContextMenuTargets,
        >,
    >,
    /// Compiled keybind configuration, parsed once from the config file at
    /// startup (change `add-configurable-keybinds`, design D3): the optional
    /// prefix chord plus per-section router overrides and prefix-namespace
    /// assignments. Shell-owned plain data the router reads as its
    /// `&Keybinds` parameter (Unit 3); never mirrored into components.
    pub keybinds: mbv_keybinds::Keybinds,
    /// The core focus stack (design D3 of `extract-client-core`, capability
    /// `client-interaction-core`): the shell-held focus authority over the
    /// mounted surfaces, the Panel focus in effect, and the overlay stack.
    /// Call sites request a change on the stack instead of calling `TuiRealm`
    /// focus directly, and the sync pass's one `apply_focus_stack` step
    /// applies the derived focus to `TuiRealm`. The stored Panel-focus record
    /// (launch restore, the mini-view interplay) stays on `App`; the stack
    /// holds the focus in effect for the derivation.
    pub(in crate::app) focus_stack: mbv_surfaces::FocusStack,
    /// Shell-owned exit kind (tray-pin-swap design D6): set once the Owner
    /// sent `SwapQuit`, so `teardown` skips the launch-state save and the
    /// coordinated shutdown request. Never reset within a run.
    pub(in crate::app) swapped_out_exit: bool,
    /// The sync pass's workspace now-playing projection
    /// (hero-workspace-now-playing D1): derived once per pass from the
    /// playing queue's displayed playback state and its active item, then
    /// read by every workspace owner's push seam. Shell-owned presentation
    /// data, never a mirror of component-local state.
    pub(in crate::app) workspace_now_playing: Option<WorkspaceNowPlaying>,
    /// Shared with the terminal input port (issue #940). The run loop releases
    /// it after each terminal tick so the port forwards the next mouse move.
    move_gate: input_port::MoveGate,
    /// The tab bar's scroll anchor (design D6b): TUI presentation, driven by
    /// [`Model::reveal_requested_tab`] from the core's `tab_reveal_requested`
    /// request and painted by `sync_tab_panel`.
    pub(in crate::app) tab_scroll: usize,
    /// The terminal size the shell last recorded, on the resize observer and
    /// on the draw path's per-frame size (design D2/D6a). Terminal geometry
    /// never reaches the core: the shell derives the single-panel fact
    /// (`App::set_single_panel`) and the queue-column clamp from these.
    pub(in crate::app) terminal_width: u16,
    pub(in crate::app) terminal_height: u16,
    /// The last fully composed frame's layout geometry (design D2/D13 Rule
    /// B): `left_area`, the queue card checkpoint, and the root panel
    /// placements. Shell presentation data built from ratatui `Rect`s, so it
    /// lives here. The core reads only the neutral copy the draw path pushes
    /// (`App::set_left_panel_area`).
    pub(in crate::app) layout: mbv_render::layout::AppLayout,
    /// The Home "new content" launch window (row 4.4 of design D2): the launch
    /// instant and the previous launch, read by the shell's destination-latest
    /// markers. Set once at `Model` construction.
    pub(in crate::app) home_latest_launch_window: HomeLatestLaunchWindow,
    /// The last sync pass that saw a live resize drag; starts the settle
    /// window (invariant 20). The live bit stays with the gesture components.
    pub(in crate::app) resize_drag_activity: Option<std::time::Instant>,
    /// This Client's terminal image protocols: the pickers, the resize worker,
    /// and the encoding of every cached key a TUI painter renders (design D7
    /// of `extract-client-core`, and design D2 rule 3 of
    /// `split-tui-gui-clients`: a store only one Client uses leaves `App`).
    /// The core queues what to encode and to drop; the shell's drains
    /// (`Model::drain_protocol_work`) act on it.
    pub(in crate::app) image_protocols: mbv_tui_support::ImageProtocols,
}

/// The single-panel fact the TUI derives from its terminal width: the width
/// is below the mini-view breakpoint (row 4.4 of design D2, from the row
/// 4.1-4.3 review). The TUI owns the threshold; the core only receives the
/// fact through `App::set_single_panel`.
pub(in crate::app) fn single_panel_for_width(width: u16) -> bool {
    width < mbv_render::layout::MINI_VIEW_THRESHOLD
}

/// Build this Client's terminal image protocol store before the `TuiRealm`
/// crossterm listener starts: the configured protocol override
/// (`ImageCache::protocol_override`) replaces the detected one, and the picker
/// facts the core reads are pushed onto `app` here.
///
/// MUST run before the `TuiRealm` crossterm listener starts (#654):
/// `Picker::from_query_stdio` writes a `CSI 16 t` cell-size query and reads the
/// reply with a raw stdin read, so a listener already draining stdin eats it.
/// The `Model` constructor takes the store pre-built, so construction order
/// makes the rule structural.
pub fn init_image_protocols(app: &mut App) -> mbv_tui_support::ImageProtocols {
    let mut image_protocols = mbv_tui_support::ImageProtocols::new();
    // The store's own override view: the override is a plain config string the
    // store already holds, so the query reads it and the facts read it back
    // from the store, with no `app` borrow outliving the call.
    image_protocols.init_image_pickers(app.images.protocol_override());
    push_image_protocol_facts(app, &image_protocols);
    image_protocols
}

/// Push the picker facts the core's projections read (design D2 rule 3):
/// whether the configured picker paints halfblocks, whether the pickers are
/// initialized, the configured picker's cell grid, the grid of the picker that
/// paints the current suffix, and the suffix the artwork cache keys carry.
///
/// Call it again whenever the dim state flips (`Model::draw_frame`): the
/// active grid follows the fallback, and the hero cover fit reads it.
pub(in crate::app) fn push_image_protocol_facts(
    app: &mut App,
    image_protocols: &mbv_tui_support::ImageProtocols,
) {
    let protocol_override = app.images.protocol_override().map(str::to_owned);
    let protocol_override = protocol_override.as_deref();
    let halfblock = image_protocols.is_halfblock_configured(protocol_override);
    let configured_font = image_protocols.image_picker().map_or((10, 20), cell_size);
    // The picker that paints the key's current suffix: the halfblock
    // fallback's while a dimming backdrop forces it (#451), else the
    // configured picker's.
    let suffix = if app.dim_halfblock_forced() {
        "halfblock"
    } else {
        image_protocols.configured_protocol_name()
    };
    let active_font = image_protocols
        .picker_for_suffix(suffix)
        .map_or((10, 20), cell_size);
    app.set_image_protocol_facts(
        halfblock,
        image_protocols.image_picker().is_some(),
        configured_font,
        active_font,
    );
    app.set_configured_protocol_suffix(image_protocols.configured_protocol_name());
}

/// A picker's terminal cell, in pixels.
fn cell_size(picker: &ratatui_image::picker::Picker) -> (u16, u16) {
    let font = picker.font_size();
    (font.width, font.height)
}

/// Install a test's pickers on a shell and push the facts the core's
/// projections read, the way `init_image_protocols` does for the terminal's
/// own pickers.
#[cfg(test)]
pub(in crate::app) fn install_image_pickers_for_test(
    model: &mut Model,
    image: ratatui_image::picker::Picker,
    halfblock: ratatui_image::picker::Picker,
) {
    model
        .image_protocols
        .set_image_pickers_for_test(image, halfblock);
    let mut app = std::mem::replace(&mut model.app, crate::app::handoff_app());
    push_image_protocol_facts(&mut app, &model.image_protocols);
    model.app = app;
}

/// The current launch instant in Unix seconds: the upper bound of the Home
/// "new content" launch window (row 4.4 of design D2). The window is shell
/// presentation state, so the clock read lives here too.
#[cfg(test)]
#[must_use]
fn current_launch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Model {
    /// The terminal size the shell last recorded (row 4.3 of design D2):
    /// terminal geometry is TUI presentation, so it lives on the shell and
    /// never reaches the core.
    pub(in crate::app) fn terminal_size(&self) -> (u16, u16) {
        (self.terminal_width, self.terminal_height)
    }

    /// Construct the model, starting the ``TuiRealm`` crossterm listener and
    /// mounting the permanent root observer.
    #[cfg(test)]
    pub fn new(app: App) -> Self {
        Self::new_with_launch_window(
            app,
            HomeLatestLaunchWindow {
                previous: None,
                current: current_launch_secs(),
            },
            mbv_tui_support::ImageProtocols::new(),
        )
    }

    /// Construct the launch shell over an already initialized protocol store
    /// (`init_image_protocols`, which must run before the listener starts;
    /// #654).
    pub(crate) fn new_with_launch_window(
        app: App,
        home_latest_launch_window: HomeLatestLaunchWindow,
        image_protocols: mbv_tui_support::ImageProtocols,
    ) -> Self {
        let move_gate = input_port::MoveGate::default();
        let listener_cfg = EventListenerCfg::default().add_port(
            Box::new(input_port::CoalescingInputPort::new(
                TERMINAL_LISTENER_INTERVAL,
                move_gate.clone(),
            )),
            TERMINAL_LISTENER_INTERVAL,
            TERMINAL_LISTENER_MAX_POLL,
        );
        Self::new_with_listener_and_window(
            app,
            listener_cfg,
            move_gate,
            home_latest_launch_window,
            image_protocols,
        )
    }

    #[cfg(test)]
    pub(in crate::app) fn new_with_listener(
        app: App,
        listener_cfg: EventListenerCfg<UserEvent>,
    ) -> Self {
        Self::new_with_listener_at(app, listener_cfg, current_launch_secs())
    }

    #[cfg(test)]
    pub(in crate::app) fn new_with_listener_at(
        app: App,
        listener_cfg: EventListenerCfg<UserEvent>,
        current_launch: u64,
    ) -> Self {
        Self::new_with_listener_and_window(
            app,
            listener_cfg,
            input_port::MoveGate::default(),
            HomeLatestLaunchWindow {
                previous: None,
                current: current_launch,
            },
            mbv_tui_support::ImageProtocols::new(),
        )
    }

    fn new_with_listener_and_window(
        app: App,
        listener_cfg: EventListenerCfg<UserEvent>,
        move_gate: input_port::MoveGate,
        home_latest_launch_window: HomeLatestLaunchWindow,
        image_protocols: mbv_tui_support::ImageProtocols,
    ) -> Self {
        let application = Application::init(listener_cfg);
        // Legacy Home Latest selector snapshots are reanchored to Continue
        // Watching by the Home owner's launch-state restoration.
        let initial_terminal_size = (80, 24);
        // The compiled `[keys]` configuration is read once, here, from the
        // config the App was built with; later config saves never rewrite it
        // for the running session.
        let keybinds = app.config.lock().unwrap().keybinds.clone();
        // The stack starts from the mounts this constructor performs below:
        // `UiRoot` and the whole-session Library panel. The Queue panel mounts
        // lazily at its first sync pass and routes its mount record to the
        // stack there (`focus.rs`).
        let mut focus_stack = mbv_surfaces::FocusStack::new();
        focus_stack.mount(ComponentId::UiRoot);
        focus_stack.mount(ComponentId::Library);
        focus_stack.set_panel_focus(app.effective_panel_focus());
        let mut model = Self {
            app,
            application,
            mouse_subscribed: std::collections::HashSet::new(),
            music_track_focus_request: None,
            pending_episode_selection: None,
            pending_music_track_selection: None,
            pending_music_reanchor: None,
            acknowledged_home_latest_sources: std::collections::HashSet::new(),
            tv_latest_snapshots: std::collections::HashMap::new(),
            emby_destination_latest_requested: std::collections::HashSet::new(),
            home_context_item: None,
            handled_terminal_size: initial_terminal_size,
            pending_terminal_resize: false,
            visual_selection: None,
            context_menu_origin: None,
            menu_pointer: None,
            menu_pointer_current: false,
            menu_pointer_press_at: None,
            context_action_snapshot: None,
            keybinds,
            focus_stack,
            swapped_out_exit: false,
            workspace_now_playing: None,
            move_gate,
            tab_scroll: 0,
            terminal_width: initial_terminal_size.0,
            terminal_height: initial_terminal_size.1,
            layout: mbv_render::layout::AppLayout::default(),
            home_latest_launch_window,
            resize_drag_activity: None,
            image_protocols,
        };
        // UiRoot owns overlay z-order and permanently observes terminal events.
        // This is the ONLY mount with a non-mouse subscription; every other
        // `.mount(` passes `vec![]` and gains mouse eligibility only via
        // `sync_mouse_subscriptions`. `fold_mouse_messages` depends on that
        // (ADR 0024) — see its doc comment before adding a subscription here.
        model
            .application
            .mount(
                ComponentId::UiRoot,
                Box::new(UiRootComponent::new()),
                UiRootComponent::subscriptions(),
            )
            .expect("mount UiRoot");
        model
            .application
            .active(&ComponentId::UiRoot)
            .expect("activate UiRoot");
        // The Library panel is mounted for the whole session (it holds every
        // library's embedded content owners, design D2's retention rule) but
        // never made active directly: the sync pass's focus step derives the
        // focused surface from the core focus stack (design D3 of
        // `extract-client-core`), and it routes to the panel while a migrated
        // library is active. It paints only a migrated owner's surface (the
        // transitional branch, task 5.9).
        // Home's and Feeds' owners are installed with the panel: both tabs
        // are always migrated, so `active_surface_id` routes them to the
        // panel from the first sync (tasks 5.11, 7.3). The other destinations
        // install owners in their conversion slices (tasks 8+).
        {
            let mut panel = mbv_components::library_panel::LibraryPanel::new();
            panel.insert_owner(
                mbv_ui_model::library::LibraryKey::Home,
                Box::new(mbv_components::home_content::HomeContent::new()),
            );
            panel.insert_owner(
                mbv_ui_model::library::LibraryKey::Feeds,
                Box::new(mbv_components::feeds_content::FeedsContent::new()),
            );
            model
                .application
                .mount(ComponentId::Library, Box::new(panel), vec![])
                .expect("mount LibraryPanel");
        };
        model
            .application
            .mount(
                ComponentId::QueueBoundary,
                Box::new(QueueBoundaryComponent::new()),
                vec![],
            )
            .expect("mount QueueBoundary");
        model.update_settings_content();
        model
    }
}

fn apply_terminal_observer(
    model: &mut Model,
    event: &TerminalObserverEvent,
    music_resize: &mut bool,
    tv_resize: &mut bool,
) {
    match event {
        TerminalObserverEvent::Resize { width, height } => {
            // The pre-resize width is only known here (task 1.2): the sync
            // pass consumes the armed flag and compares it against the size
            // it last handled.
            model.pending_terminal_resize = true;
            model.terminal_width = *width;
            model.terminal_height = *height;
            let single_panel = single_panel_for_width(*width);
            model.app.set_single_panel(single_panel);
            // Invariant 17: a resize neither blanks the terminal nor wipes
            // the image cache. A `force_clear` here blanked the panel in the
            // middle of pinwin's width tween, and wiping the cache made every
            // visible card and the hero refetch and re-encode; ratatui's own
            // autoresize repaint and the painters' protocol resize path
            // already cover the new geometry.
            model.push_inline_search_content();
            *music_resize = true;
            *tv_resize = true;
        }
        TerminalObserverEvent::FocusGained => model.app.note_focus_gained(),
        TerminalObserverEvent::FocusLost => model.app.note_focus_lost(),
        // Task 6.5's `MouseClick` observer signal resolved shell-painted tab
        // chrome; the tab bar is a mounted `TabPanel` now (task 2.1) and the
        // click reaches it through its `mouse_sub()` subscription, so a
        // click here is only the observer's redraw echo (no shell geometry
        // is read). It is still a mouse event: any mouse event silently
        // disarms prefix mode (design D6, task 6.1) — a shell-side flag
        // clear plus focus restore; the mouse event's own delivery and
        // handling are unchanged.
        // A right press on any surface records the context-menu pointer the
        // shell places a pointer-anchored menu against (design D6). The tick
        // already recorded it before draining any message
        // (`Model::record_right_press`), because the focused panel's own
        // request arrives first; this repeats the record for a tick that
        // reaches the observer by another route. It does not mark it current:
        // `Model::scope_menu_pointer` derives that per message, so the report
        // itself never rescues a keyboard request in the same tick. It is also
        // a mouse event, so it disarms prefix mode like every other.
        TerminalObserverEvent::MouseRightClick { column, row } => {
            model.menu_pointer = Some((*column, *row));
            model.disarm_prefix_mode();
        }
        TerminalObserverEvent::MouseClick { .. } | TerminalObserverEvent::Mouse => {
            model.disarm_prefix_mode();
        }
        TerminalObserverEvent::Key(_)
        | TerminalObserverEvent::NoOp
        | TerminalObserverEvent::MouseClaimed
        | TerminalObserverEvent::KeyClaimed => {}
    }
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
