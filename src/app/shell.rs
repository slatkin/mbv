use std::time::Duration;

use super::components::msg::AlbumCursorKind;
use super::components::{
    ComponentId, Msg, OverlayId, QueueBoundaryComponent, ShellRequest, TerminalObserverEvent,
    UiRootComponent, UserEvent,
};
use super::input::router::{resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot};
use super::{
    components, palette, render, AlbumIndexState, App, BrowseLevel, ConfirmAction, ConfirmModal,
    IdleFeed, LibEvent, PanelFocus, PanelMode, PlaybackState, PlayerTab, QueueScope,
    SavePlaylistDialog, SavePlaylistStage, SidebarId, TabSelection, ToastSeverity,
};
use super::{
    init_terminal, install_signal_handlers, restore_terminal, start_quit_watchdog, QUIT_REQUESTED,
};
use crate::app::dispatch::action::Command;
use crate::app::dispatch::session::service_startup;
#[cfg(test)]
use crate::app::state::home_latest::current_launch_secs;
use crate::app::state::home_latest::HomeLatestLaunchWindow;
use crate::app::state::types::feeds_manage::FeedsManagePopup;
use crate::app::state::types::playback::{
    DestinationLatestSnapshot, DestinationLatestSource, HomeContent,
};
use tuirealm::application::{Application, PollStrategy};
use tuirealm::listener::EventListenerCfg;

mod audiobookshelf_book;
mod audiobookshelf_podcast;
mod chrome_panels;
mod draw;
mod emby_library;
mod emby_library_content;
mod feeds;
mod feeds_manage;
mod home;
mod home_content;
mod inline_search;
mod library;
mod library_panel;
mod messages;
mod modal_actions;
mod music_workspace;
mod overlays;
mod playback;
mod playlists;
mod queue;
mod root;
mod run;
mod settings;
mod tv_workspace;

/// How often the TuiRealm crossterm listener worker polls the terminal for
/// events. The listener's `poll` blocks for half of this; the worker cycle is
/// this long. Set to 8 ms so event latency matches the legacy loop's fastest
/// cadence (the visualizer's 8 ms poll). The main thread's per-iteration wait
/// is governed separately by the `PollStrategy::Once` timeout below, so this
/// only affects how promptly a buffered event reaches the channel — not the
/// render cadence.
const TERMINAL_LISTENER_INTERVAL: Duration = Duration::from_millis(8);
/// Upper bound on events the listener drains from crossterm in one worker
/// cycle. Generous so a burst (e.g. a mouse drag) is flushed into the channel
/// in one cycle; the main thread still processes at most one per tick via
/// `PollStrategy::Once`, matching the legacy one-event-per-iteration loop.
const TERMINAL_LISTENER_MAX_POLL: usize = 60;

/// A second Esc inside this window stops playback; a single Esc stays free
/// for whatever claims it (sidebar, search, overlay dismissal).
const DOUBLE_ESC_STOP_WINDOW: Duration = Duration::from_millis(600);

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

/// Shell model holding the legacy `App` and the TuiRealm `Application`.
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
    /// One-shot shell→component re-anchor trigger for the mounted Music
    /// workspace's album cursor/scroll, consumed at the next
    /// `push_music_workspace_content`. Set at the three navigation events that
    /// legitimately move a shell-owned cursor -- group switch, recursive-album
    /// activation, saved-position restore -- and once after mount. An ordinary
    /// content push never adopts the shell cursor; this is the explicit
    /// re-anchor that replaced the deleted echo-suppression test.
    pub(in crate::app) music_workspace_reanchor: bool,
    /// Shell-owned mirror of the feeds-management popup's interaction state
    /// plus its background add-feed channel (task 5.3c). The
    /// `FeedsManageComponent` mirrors `stage`/`cursor`/`feeds`/`pending_add`
    /// from here each tick; the mpsc cannot live in the component.
    pub(in crate::app) feeds_manage: Option<FeedsManagePopup>,
    /// Model-owned Home content (task 5.3d): the sole snapshot pushed to
    /// `HomeComponent`; App-internal writers deliver computed snapshots via
    /// lib_tx; `loading` mirrors the deleted `App.home_loading`.
    pub(in crate::app) home_content: HomeContent,
    /// Shell-owned acknowledgement shared by Home and TV Latest surfaces.
    pub(in crate::app) acknowledged_home_latest_sources:
        std::collections::HashSet<DestinationLatestSource>,
    /// One authoritative TV Latest section snapshot per Emby library view.
    pub(in crate::app) tv_latest_snapshots:
        std::collections::HashMap<String, DestinationLatestSnapshot>,
    pub(in crate::app) home_context_item: Option<mbv_core::api::EmbyItem>,
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
    /// Fingerprint of the inputs `sync_queue` last projected into the mounted
    /// `QueueComponent`. `sync_queue` runs every run-loop tick; rebuilding the
    /// row vec (slot clone + per-row `format!`) on a tick where nothing the
    /// projection depends on changed is pure waste (#675). The fingerprint
    /// gates the rebuild: queue revision + viewed scope + active slot + a
    /// progress-% bucket + paused + the title model.
    pub(in crate::app) last_queue_projection: Option<queue::QueueProjectionFingerprint>,
    /// Shell-owned projection of the focused list's Visual selection.
    pub(in crate::app) visual_selection:
        Option<(crate::app::state::types::settings::PanelFocus, usize)>,
    pub(in crate::app) context_menu_origin:
        Option<crate::app::components::media_list::SelectionOrigin>,
    pub(in crate::app) context_action_snapshot: Option<
        crate::app::state::types::context_menu::ContextActionSnapshot<
            crate::app::state::types::context_menu::ContextMenuTargets,
        >,
    >,
    /// The last Esc press, for the double-Esc playback stop. Shell-owned
    /// timing state: a single Esc falls through to its claimants, and only
    /// a second Esc within [`DOUBLE_ESC_STOP_WINDOW`] dispatches the stop.
    pub(in crate::app) last_esc: Option<std::time::Instant>,
    /// Compiled keybind configuration, parsed once from the config file at
    /// startup (change `add-configurable-keybinds`, design D3): the optional
    /// prefix chord plus per-section router overrides and prefix-namespace
    /// assignments. Shell-owned plain data the router reads as its
    /// `&Keybinds` parameter (Unit 3); never mirrored into components.
    pub keybinds: mbv_core::keybinds::Keybinds,
    /// The focus displaced by arming prefix mode (design D6, task 6.1).
    /// tuirealm forwards every chord to the focused component before the
    /// router fold, so armed capture blurs focus for the armed ticks; this
    /// records what was focused when the shell armed, and the disarm
    /// restores it (falling back to the sync passes' canonical
    /// re-derivation when it was unmounted meanwhile).
    pub(in crate::app) prefix_armed_focus: Option<ComponentId>,
}

/// The ADR 0023 Keyboard Router fold: apply the router's outcome to this
/// tick's message list and return the messages that survive.
///
/// `Application::tick` returns the focused component's message first, then the
/// UiRoot observer's `TerminalEvent`. With `PollStrategy::Once` there is at
/// most one terminal event per tick, so the messages for a key chord are:
///
/// * **UiRoot focused** — only the observer's `TerminalEvent(Key)`. This is
///   the active component's own message; `FallThrough` keeps it, while
///   `Command`/`Swallow` replace it (the command is dispatched by the caller).
/// * **Leaf focused** — the leaf's request (or `None`) plus the observer's
///   `TerminalEvent(Key)`. The router's outcome selects between them:
///   `FallThrough` keeps the leaf's request; `Command`/`Swallow` discard it.
///
/// Non-key observer signals (`Resize`, `FocusGained/Lost`, `NoOp`) always pass
/// through: they are redraw/layout signals, not chords.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct ArbitrationDiagnostic {
    pub chord: Option<String>,
    pub captured_focus: Option<String>,
    pub router_result: String,
    pub leaf_disposition: &'static str,
    pub final_disposition: &'static str,
    pub dispatch_kind: &'static str,
}

/// Test seam over `arbitrate_key`: the folded message list without the
/// diagnostic record. Production routes through `arbitrate_key` directly.
#[cfg(test)]
pub(in crate::app) fn fold_keyboard_messages(
    messages: Vec<Msg>,
    focused: Option<&ComponentId>,
    router: &RouterOutcome,
) -> Vec<Msg> {
    arbitrate_key(messages, focused, router).0
}

/// Pure arbitration seam, returning both surviving messages and its compact
/// diagnostic record (never containing request payloads).
pub(in crate::app) fn arbitrate_key(
    messages: Vec<Msg>,
    focused: Option<&ComponentId>,
    router: &RouterOutcome,
) -> (Vec<Msg>, ArbitrationDiagnostic) {
    let (chord, observed_key, claims) = observe_key_messages(&messages);
    let out = retain_arbitrated_messages(messages, focused, router, observed_key);
    let diagnostic = arbitration_diagnostic(chord, focused, router, claims, &out);
    (out, diagnostic)
}

fn observe_key_messages(messages: &[Msg]) -> (Option<String>, bool, usize) {
    let chord = messages.iter().find_map(|m| match m {
        Msg::TerminalEvent(TerminalObserverEvent::Key(key)) => Some(format!("{key:?}")),
        _ => None,
    });
    let key_count = messages
        .iter()
        .filter(|m| matches!(m, Msg::TerminalEvent(TerminalObserverEvent::Key(_))))
        .count();
    debug_assert!(
        key_count <= 1,
        "malformed arbitration: duplicate router observations"
    );
    let claims = messages
        .iter()
        .filter(|m| matches!(m, Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed)))
        .count();
    debug_assert!(
        claims <= 1,
        "malformed arbitration: multiple focused key claims"
    );
    (chord, key_count == 1, claims)
}

fn retain_arbitrated_messages(
    messages: Vec<Msg>,
    focused: Option<&ComponentId>,
    router: &RouterOutcome,
    observed_key: bool,
) -> Vec<Msg> {
    let mut out = Vec::with_capacity(messages.len());
    for msg in messages {
        match msg {
            Msg::TerminalEvent(TerminalObserverEvent::Key(_)) => {
                if focused == Some(&ComponentId::UiRoot)
                    && matches!(
                        router,
                        RouterOutcome::FallThrough | RouterOutcome::Deferred(_)
                    )
                {
                    out.push(msg);
                }
            }
            Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed) => {
                debug_assert!(
                    observed_key,
                    "malformed arbitration: key claim without observer event"
                );
            }
            Msg::TerminalEvent(_) => out.push(msg),
            leaf => {
                if !observed_key
                    || matches!(
                        router,
                        RouterOutcome::FallThrough | RouterOutcome::Deferred(_)
                    )
                {
                    out.push(leaf);
                }
            }
        }
    }
    out
}

fn arbitration_diagnostic(
    chord: Option<String>,
    focused: Option<&ComponentId>,
    router: &RouterOutcome,
    claims: usize,
    out: &[Msg],
) -> ArbitrationDiagnostic {
    let leaf_disposition = if claims > 0 || out.iter().any(|m| !matches!(m, Msg::TerminalEvent(_)))
    {
        "consumed"
    } else {
        "unhandled"
    };
    let final_disposition = match router {
        RouterOutcome::Command(_) => "command",
        RouterOutcome::PrefixDispatch(_) => "prefix-dispatch",
        RouterOutcome::PrefixArm => "prefix-arm",
        RouterOutcome::PrefixSwallow => "prefix-swallow",
        RouterOutcome::Swallow => "swallow",
        RouterOutcome::FallThrough => {
            if leaf_disposition == "consumed" {
                "fall-through-consumed"
            } else {
                "fall-through"
            }
        }
        RouterOutcome::Deferred(_) => "deferred",
    };
    let dispatch_kind = match router {
        RouterOutcome::Command(_) | RouterOutcome::PrefixDispatch(_) => "command",
        _ if out.iter().any(|m| !matches!(m, Msg::TerminalEvent(_))) => "request",
        _ => "none",
    };
    ArbitrationDiagnostic {
        chord,
        captured_focus: focused.map(|f| format!("{f:?}")),
        router_result: format!("{router:?}"),
        leaf_disposition,
        final_disposition,
        dispatch_kind,
    }
}

/// ADR 0024: the mouse fold, applied to a `tick()` message list beside the
/// ADR 0023 keyboard router fold.
///
/// `tuirealm` forwards `Event::Mouse` to the focused component *and* to every
/// subscriber, so a mouse tick can yield several `Msg`s. `sync_mouse_subscriptions`
/// keeps only components painted this frame mouse-eligible, and those paint
/// disjoint rectangles and each emits only for points inside its own geometry,
/// so at most one should claim any event. This fold applies the first claim in
/// `tick()` order and `debug_assert!`s that no more than one was produced — two
/// claims for one event is a geometry defect, not a ranking problem, so the fold
/// deliberately has no `Msg`-variant → surface table.
///
/// Keyboard ticks carry a `TerminalObserverEvent::Key` marker (the permanent
/// `UiRoot` observer emits it for every `Event::Keyboard`); those pass through
/// untouched for the keyboard router to resolve.
///
/// Load-bearing invariant: `UiRootComponent` is the *only* component that mounts
/// with a non-mouse (`EventClause::Any`) subscription — every other `.mount(`
/// site passes `vec![]`, and mouse eligibility is added later by
/// `sync_mouse_subscriptions`. The structural mouse-tick detection here (a tick
/// with no `TerminalEvent` marker is a mouse tick) depends on that: if a future
/// component mounts its own non-mouse subscription, a non-mouse tick could reach
/// this fold with no marker and be mistaken for a mouse claim — release builds
/// drop it silently, debug builds trip the `debug_assert!` below (ADR 0024).
pub(in crate::app) fn fold_mouse_messages(messages: Vec<Msg>) -> Vec<Msg> {
    let observed_key = messages
        .iter()
        .any(|msg| matches!(msg, Msg::TerminalEvent(TerminalObserverEvent::Key(_))));
    if observed_key {
        return messages;
    }
    let claims = messages
        .iter()
        .filter(|msg| {
            !matches!(
                msg,
                Msg::TerminalEvent(
                    TerminalObserverEvent::NoOp
                        | TerminalObserverEvent::Key(_)
                        | TerminalObserverEvent::Resize { .. }
                        | TerminalObserverEvent::FocusGained
                        | TerminalObserverEvent::FocusLost
                        | TerminalObserverEvent::Mouse
                        | TerminalObserverEvent::MouseClick { .. }
                        | TerminalObserverEvent::MouseClaimed
                        | TerminalObserverEvent::KeyClaimed
                )
            )
        })
        .count();
    debug_assert!(
        claims <= 1,
        "mouse fold: {claims} components claimed one mouse event; eligible \
         surfaces paint disjoint regions (ADR 0024)"
    );
    let mut kept_claim = false;
    messages
        .into_iter()
        .filter(|msg| {
            if matches!(msg, Msg::TerminalEvent(_)) {
                return true;
            }
            let first = !kept_claim;
            kept_claim = true;
            first
        })
        .collect()
}

impl Model {
    /// Build the router snapshot and resolve the terminal chord. The router
    /// reads a plain-data snapshot, never component attributes (ADR 0023).
    pub(in crate::app) fn router_outcome(&mut self, messages: &[Msg]) -> RouterOutcome {
        let Some(tui_key) = messages.iter().find_map(|msg| match msg {
            Msg::TerminalEvent(TerminalObserverEvent::Key(key)) => Some(*key),
            _ => None,
        }) else {
            return RouterOutcome::FallThrough;
        };
        let key = super::input::resolver::tuirealm_key_to_crossterm(tui_key);
        // Double-Esc stop: a single Esc must stay free for whatever claims
        // it (sidebar, search, overlay dismissal), so the stop dispatches
        // only when this Esc follows another Esc inside the window. The
        // tracker updates before resolution; the outcome gate below is the
        // only consumer.
        let now = std::time::Instant::now();
        let double_esc = matches!(
            self.last_esc,
            Some(last) if now.duration_since(last) <= DOUBLE_ESC_STOP_WINDOW
        );
        self.last_esc = (key.code == crossterm::event::KeyCode::Esc).then_some(now);
        // `player.status` is a plain (non-reentrant) mutex and `RouterSnapshot`
        // initializers below call `effective_playback_state()`, which locks it
        // again. A temporary created anywhere inside the struct literal lives
        // until the whole `let snapshot = ...;` statement ends, so taking that
        // lock inline self-deadlocked the run loop on the first key press in
        // any QueueOnly/mini-view frame. Read it in its own statement.
        let player_active = self.app.player.status.lock().unwrap().active;
        let snapshot = RouterSnapshot {
            player_active,
            has_remote_session: self.app.connected_session_id.is_some()
                || self.app.player.is_remote()
                || self.app.is_cast_attached(),
            connected_session_id_present: self.app.connected_session_id.is_some(),
            // Task 3.8: the idle-feed open-link gate follows the Queue
            // playback panel's presence, not the panel mode. The panel is
            // mounted in every queue-visible layout (idle included), so the
            // link is gated off whenever the queue column is visible and
            // nothing is playing; in library-only the Library playback
            // panel's strip displays the idle feed and the link opens.
            queue_only_idle: self.application.mounted(&ComponentId::QueuePlaybackPanel)
                && !self.app.effective_playback_state().active,
            panel_mode: self.app.effective_panel_mode(),
            panel_focus: self.app.effective_panel_focus(),
            overlay_holds_focus: self.overlay_holds_focus(),
            blocking_overlay_open: self.blocking_overlay_active(),
            help_overlay_open: self
                .application
                .mounted(&ComponentId::Overlay(OverlayId::Help)),
            sessions_sidebar_open: self
                .application
                .mounted(&ComponentId::Overlay(OverlayId::Sessions)),
            context_menu_open: self
                .application
                .mounted(&ComponentId::Overlay(OverlayId::ContextMenu)),
            idle_feed_link_available: self.app.idle_feed_link_available(),
            text_entry_focused: matches!(
                self.application.focus(),
                Some(
                    ComponentId::Overlay(OverlayId::Search)
                        | ComponentId::Overlay(OverlayId::Settings)
                )
            ) || self.active_inline_search_is_open(),
            prefix_armed: self.app.prefix_armed,
        };

        let outcome = resolve_router_outcome_with_focused(
            key,
            &snapshot,
            self.application.focus(),
            &self.keybinds,
        );
        // The first Esc resolves to the stop candidate but falls through:
        // the leaf (and the deferred-candidate arbitration) handle it as
        // today, and nothing dispatches. A non-Esc stop chord (a rebound
        // binding) fires immediately, unchanged.
        let outcome = match outcome {
            RouterOutcome::Deferred(Command::Stop) if !double_esc => RouterOutcome::FallThrough,
            other => other,
        };
        // Arm/disarm transitions come from the router's outcome (design D6,
        // task 6.1): the arming layer arms, an armed dispatch (mapped or
        // swallowed) disarms. The prefix chord re-arms through `PrefixArm`.
        match outcome {
            RouterOutcome::PrefixArm => self.arm_prefix_mode(),
            RouterOutcome::PrefixDispatch(_) | RouterOutcome::PrefixSwallow => {
                self.disarm_prefix_mode();
            }
            _ => {}
        }
        outcome
    }

    /// Arm prefix mode (design D6, task 6.1): set the App-owned bit and
    /// withhold the keyboard from the focused leaf for the armed ticks.
    /// tuirealm forwards every chord to the focused component before the
    /// router fold (`Application::tick` → `forward_to_active_component`),
    /// so armed capture — no chord reaches any component — blurs focus for
    /// the armed ticks, exactly how blocking overlays withhold underlying
    /// input. The displaced focus is saved for the disarm restore; the
    /// focus-managing sync passes gate on the armed bit so they never steal
    /// focus back mid-capture. The arm tick's own chord was already
    /// delivered to the leaf before this runs — unavoidable, and the prefix
    /// chord performs no other action at the router.
    pub(in crate::app) fn arm_prefix_mode(&mut self) {
        if self.app.prefix_armed {
            return;
        }
        self.prefix_armed_focus = self.application.focus().cloned();
        self.app.prefix_armed = true;
        if self.application.focus().is_some() {
            self.application.blur().expect("blur for prefix arm");
        }
    }

    /// Disarm prefix mode: clear the App-owned bit and restore the focus the
    /// arm displaced. A mouse-event disarm restores it too, so the event is
    /// handled exactly as if prefix mode had never been armed.
    pub(in crate::app) fn disarm_prefix_mode(&mut self) {
        if !self.app.prefix_armed {
            return;
        }
        self.app.prefix_armed = false;
        if let Some(id) = self.prefix_armed_focus.take() {
            if self.application.mounted(&id) {
                self.application
                    .active(&id)
                    .expect("restore prefix-displaced focus");
            }
        }
    }

    /// Apply a deferred candidate after the focused leaf has been arbitrated.
    ///
    /// A deferred candidate carries no timing state (semantic-input-arbitration):
    /// an unhandled press dispatches the command on that press; a consumed press
    /// cancels the candidate and records nothing, so a later unhandled press
    /// behaves as a first press. Returns whether the candidate fired. The
    /// shell's quit signal is unaffected: deferred candidates are only ever
    /// `TogglePlayPause`/`Stop`, which never quit.
    pub(in crate::app) fn apply_deferred_candidate(
        &mut self,
        router: &RouterOutcome,
        leaf_consumed: bool,
    ) -> bool {
        let RouterOutcome::Deferred(command) = router else {
            return false;
        };
        if leaf_consumed {
            return false;
        }
        let quit = self.dispatch_router_command(command.clone());
        debug_assert!(!quit, "deferred candidates never quit");
        true
    }

    pub(in crate::app) fn dispatch_router_command(&mut self, command: Command) -> bool {
        match command {
            Command::OpenHelp => {
                self.mount_help();
                false
            }
            command => self.app.dispatch(command),
        }
    }

    /// Construct the model, starting the TuiRealm crossterm listener and
    /// mounting the permanent root observer.
    #[cfg(test)]
    pub fn new(app: App) -> Self {
        Self::new_with_listener(
            app,
            EventListenerCfg::default()
                .crossterm_input_listener(TERMINAL_LISTENER_INTERVAL, TERMINAL_LISTENER_MAX_POLL),
        )
    }

    pub(crate) fn new_with_launch_window(
        app: App,
        home_latest_launch_window: HomeLatestLaunchWindow,
    ) -> Self {
        Self::new_with_listener_and_window(
            app,
            EventListenerCfg::default()
                .crossterm_input_listener(TERMINAL_LISTENER_INTERVAL, TERMINAL_LISTENER_MAX_POLL),
            home_latest_launch_window,
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
            HomeLatestLaunchWindow {
                previous: None,
                current: current_launch,
            },
        )
    }

    fn new_with_listener_and_window(
        mut app: App,
        listener_cfg: EventListenerCfg<UserEvent>,
        home_latest_launch_window: HomeLatestLaunchWindow,
    ) -> Self {
        let application = Application::init(listener_cfg);
        app.home_latest_launch_window = home_latest_launch_window;
        // Legacy Home Latest selector snapshots are reanchored to Continue
        // Watching by the Home owner's launch-state restoration.
        let initial_terminal_size = (app.terminal_width, app.terminal_height);
        // The compiled `[keys]` configuration is read once, here, from the
        // config the App was built with; later config saves never rewrite it
        // for the running session.
        let keybinds = app.config.lock().unwrap().keybinds.clone();
        let mut model = Self {
            app,
            application,
            mouse_subscribed: std::collections::HashSet::new(),
            music_track_focus_request: None,
            pending_episode_selection: None,
            pending_music_track_selection: None,
            music_workspace_reanchor: false,
            feeds_manage: None,
            home_content: HomeContent::new(),
            acknowledged_home_latest_sources: std::collections::HashSet::new(),
            tv_latest_snapshots: std::collections::HashMap::new(),
            home_context_item: None,
            handled_terminal_size: initial_terminal_size,
            pending_terminal_resize: false,
            last_queue_projection: None,
            visual_selection: None,
            context_menu_origin: None,
            context_action_snapshot: None,
            last_esc: None,
            keybinds,
            prefix_armed_focus: None,
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
        // never made active directly: `sync_active_destination` routes focus
        // to it while a migrated library is active, and it paints only a
        // migrated owner's surface (the transitional branch, task 5.9).
        // Home's and Feeds' owners are installed with the panel: both tabs
        // are always migrated, so `active_surface_id` routes them to the
        // panel from the first sync (tasks 5.11, 7.3). The other destinations
        // install owners in their conversion slices (tasks 8+).
        {
            let mut panel = super::components::library_panel::LibraryPanel::new();
            panel.insert_owner(
                super::components::library_panel::LibraryKey::Home,
                Box::new(super::components::home_content::HomeContent::new()),
            );
            panel.insert_owner(
                super::components::library_panel::LibraryKey::Feeds,
                Box::new(super::components::feeds_content::FeedsContent::new()),
            );
            model
                .application
                .mount(ComponentId::Library, Box::new(panel), vec![])
                .expect("mount LibraryPanel");
        }
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
    event: TerminalObserverEvent,
    music_resize: &mut bool,
    tv_resize: &mut bool,
) {
    match event {
        TerminalObserverEvent::Resize { width, height } => {
            // The pre-resize width is only known here (task 1.2): the sync
            // pass consumes the armed flag and compares it against the size
            // it last handled.
            model.pending_terminal_resize = true;
            model.app.terminal_width = width;
            model.app.terminal_height = height;
            model.app.force_clear = true;
            model.app.card_image_states.clear();
            model.app.card_image_loading.clear();
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
        TerminalObserverEvent::MouseClick { .. } => model.disarm_prefix_mode(),
        TerminalObserverEvent::Mouse => model.disarm_prefix_mode(),
        TerminalObserverEvent::Key(_)
        | TerminalObserverEvent::NoOp
        | TerminalObserverEvent::MouseClaimed
        | TerminalObserverEvent::KeyClaimed => {}
    }
}

#[cfg(test)]
mod mouse_fold_tests {
    use super::*;
    use crate::app::components::msg::PlaybackRequest;

    #[test]
    fn fold_keeps_one_mouse_claim_and_the_observer_signal() {
        let msgs = vec![
            Msg::Playback(PlaybackRequest::TogglePlayPause),
            Msg::TerminalEvent(TerminalObserverEvent::NoOp),
        ];
        assert_eq!(
            fold_mouse_messages(msgs),
            vec![
                Msg::Playback(PlaybackRequest::TogglePlayPause),
                Msg::TerminalEvent(TerminalObserverEvent::NoOp),
            ]
        );
    }

    #[test]
    fn duplicate_mouse_observer_markers_do_not_hide_a_claim() {
        let marker = Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed);
        let claim = Msg::Playback(PlaybackRequest::TogglePlayPause);
        assert_eq!(
            fold_mouse_messages(vec![marker.clone(), marker.clone(), claim.clone()]),
            vec![marker.clone(), marker, claim]
        );
    }

    #[test]
    fn fold_passes_a_keyboard_tick_through_untouched() {
        use tuirealm::event::{Key, KeyEvent, KeyModifiers};
        let key = Msg::TerminalEvent(TerminalObserverEvent::Key(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        let leaf = Msg::Playback(PlaybackRequest::TogglePlayPause);
        assert_eq!(
            fold_mouse_messages(vec![leaf.clone(), key.clone()]),
            vec![leaf, key]
        );
    }
}

#[cfg(test)]
mod tests;
