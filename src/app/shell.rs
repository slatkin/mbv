use std::time::{Duration, Instant};

use super::action::{playback_command_for_key, Command};
use super::components::msg::{AlbumCursorKind, BrowserHitRegion, QueueHitRegion, TvHitRegion};
use super::components::{
    ComponentId, Msg, OverlayId, PlaybackComponent, ShellRequest, TerminalObserverEvent,
    UiRootComponent, UserEvent,
};
use super::router::{resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot};
use super::service_startup;
use super::types_feeds_manage::FeedsManagePopup;
use super::types_playback::{HomeContent, HomeLatestSource};
use super::{
    init_terminal, install_signal_handlers, restore_terminal, start_quit_watchdog, QUIT_REQUESTED,
};
use super::{App, IdleFeed, QueueScope, ToastSeverity};
use crossterm::event::KeyCode;
use tuirealm::application::{Application, PollStrategy};
use tuirealm::listener::EventListenerCfg;

#[path = "shell_messages.rs"]
mod shell_messages;
#[path = "shell_run.rs"]
mod shell_run;

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

/// Shell model holding the legacy `App` and the TuiRealm `Application`.
pub struct Model {
    pub app: App,
    pub(super) application: Application<ComponentId, Msg, UserEvent>,
    pub(super) emby_browser_id: Option<ComponentId>,
    pub(super) tv_workspace_id: Option<ComponentId>,
    pub(super) music_workspace_id: Option<ComponentId>,
    pub(super) abs_podcast_id: Option<ComponentId>,
    pub(super) abs_book_id: Option<ComponentId>,
    /// Maintained registry of every mounted destination surface component
    /// (`Browser` workspaces and `InlineSearch`). TuiRealm's `Application`
    /// exposes no component enumeration, so stale-discovery for
    /// reconciliation cannot read the view registry; this set mirrors every
    /// destination `mount`/`umount` (tasks 1.2 correction) so
    /// `reconcile_destination_mounts` can find a retired library's component
    /// even when no `*_id` pointer still names it.
    pub(super) mounted_destinations: std::collections::HashSet<ComponentId>,
    /// One-shot shell→component request for the mounted Music workspace's
    /// inline track focus, applied at the next `sync_music_workspace` after
    /// the component is mounted/synced (so mount-timing never loses it).
    /// `Some(true)` = enter focus (recursive album activation);
    /// `Some(false)` = clear focus (position restore). Neither mirrors App
    /// state: the component owns the cursor, the shell only delivers the
    /// trigger that used to write the deleted inline track-focus field.
    pub(super) music_track_focus_request: Option<bool>,
    /// One-shot shell→component re-anchor trigger for the mounted Music
    /// workspace's album cursor/scroll, consumed at the next
    /// `push_music_workspace_content`. Set at the three navigation events that
    /// legitimately move a shell-owned cursor -- group switch, recursive-album
    /// activation, saved-position restore -- and once after mount. An ordinary
    /// content push never adopts the shell cursor; this is the explicit
    /// re-anchor that replaced the deleted echo-suppression test.
    pub(super) music_workspace_reanchor: bool,
    /// One-shot shell→component re-anchor trigger for the mounted wide TV
    /// workspace's series cursor/scroll, consumed at the next
    /// `push_tv_workspace_content`. Set by the breakpoint hand-off
    /// (`hand_off_tv_breakpoint`, migrate-narrow-browse task 2.3 / D5) when
    /// the active-destination pointer flips from the narrow `BrowserComponent`
    /// to `TvWorkspaceComponent`, so the kept-mounted workspace adopts the
    /// resting position the narrow browser left behind instead of its stale
    /// local cursor.
    pub(super) tv_workspace_reanchor: bool,
    /// Shell-owned mirror of the feeds-management popup's interaction state
    /// plus its background add-feed channel (task 5.3c). The
    /// `FeedsManageComponent` mirrors `stage`/`cursor`/`feeds`/`pending_add`
    /// from here each tick; the mpsc cannot live in the component.
    pub(super) feeds_manage: Option<FeedsManagePopup>,
    /// Model-owned Home content (task 5.3d): the sole snapshot pushed to
    /// `HomeComponent`; App-internal writers deliver computed snapshots via
    /// lib_tx; `loading` mirrors the deleted `App.home_loading`.
    pub(super) home_content: HomeContent,
    /// Shell-owned semantic Home section preference and one-time restore marker.
    pub(super) home_section_pref_semantic: Option<HomeLatestSource>,
    pub(super) home_section_pending: Option<HomeLatestSource>,
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
pub(super) fn apply_router_outcome(
    messages: Vec<Msg>,
    focused: Option<&ComponentId>,
    router: &RouterOutcome,
) -> Vec<Msg> {
    let observed_key = messages
        .iter()
        .any(|msg| matches!(msg, Msg::TerminalEvent(TerminalObserverEvent::Key(_))));
    let mut out = Vec::with_capacity(messages.len());
    for msg in messages {
        match msg {
            Msg::TerminalEvent(TerminalObserverEvent::Key(_)) => {
                // The observed chord. When UiRoot itself is focused this is
                // the leaf message (the active component's own request) and
                // its survival is decided by the router like any leaf message.
                if focused == Some(&ComponentId::UiRoot) {
                    match router {
                        RouterOutcome::FallThrough => out.push(msg),
                        RouterOutcome::Command(_) | RouterOutcome::Swallow => {}
                    }
                }
                // When a leaf is focused the observer key is only the router's
                // trigger; the fold already applied the outcome to the leaf's
                // own message below.
            }
            Msg::TerminalEvent(_) => out.push(msg),
            leaf => {
                // The focused component's request (or a typed shell request
                // from a subscription). `FallThrough` lets it stand; the
                // router's `Command`/`Swallow` discards it for this tick.
                // With no key observed, nothing was routed and every message
                // stands.
                match (router, observed_key) {
                    (RouterOutcome::FallThrough, _) | (_, false) => out.push(leaf),
                    (RouterOutcome::Command(_) | RouterOutcome::Swallow, true) => {}
                }
            }
        }
    }
    out
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
        let key = super::input_resolver::tuirealm_key_to_crossterm(tui_key);

        let snapshot = RouterSnapshot {
            player_active: self.app.player.status.lock().unwrap().active,
            has_remote_session: self.app.connected_session_id.is_some()
                || self.app.player.is_remote()
                || self.app.is_cast_attached(),
            connected_session_id_present: self.app.connected_session_id.is_some(),
            panel_mode: self.app.effective_panel_mode(),
            panel_focus: self.app.effective_panel_focus(),
            blocking_overlay_open: self.is_blocking_overlay_open(),
            help_overlay_open: self
                .application
                .mounted(&ComponentId::Overlay(OverlayId::Help)),
            selection_modal_open: self
                .application
                .mounted(&ComponentId::Overlay(OverlayId::SelectionModal)),
            context_menu_open: self
                .application
                .mounted(&ComponentId::Overlay(OverlayId::ContextMenu)),
            idle_feed_link_available: self.app.idle_feed_link_available(),
            text_entry_focused: matches!(
                self.application.focus(),
                Some(
                    ComponentId::Overlay(OverlayId::Search)
                        | ComponentId::Overlay(OverlayId::Settings)
                ) | Some(ComponentId::InlineSearch(_))
            ),
            space_double_tap: self
                .app
                .last_space_press
                .is_some_and(|pressed| pressed.elapsed() < Duration::from_millis(300)),
            esc_double_tap: self
                .app
                .last_esc_press
                .is_some_and(|pressed| pressed.elapsed() < Duration::from_millis(300)),
        };

        let outcome = resolve_router_outcome_with_focused(key, &snapshot, self.application.focus());
        // The router arms the double-tap timer on the first eligible Space/Esc
        // press regardless of focus; the second press within the window is
        // claimed by `command_for_policy` when the double-tap snapshot flag is
        // set.
        self.update_double_tap_state(key, &snapshot, &outcome);
        outcome
    }

    /// Keep the existing App-owned double-tap timestamps in sync while the
    /// router owns playback resolution. A first eligible press falls through
    /// to the focused leaf and starts its timer; a second press is claimed by
    /// the router and clears the timer after dispatch is selected.
    fn update_double_tap_state(
        &mut self,
        key: crossterm::event::KeyEvent,
        snapshot: &RouterSnapshot,
        outcome: &RouterOutcome,
    ) {
        let playback = playback_command_for_key(
            super::input_resolver::KeyChord::from_key(key),
            snapshot.player_active,
            snapshot.has_remote_session,
        );
        match (key.code, playback, outcome) {
            (KeyCode::Char(' '), Some(Command::TogglePlayPause), RouterOutcome::FallThrough)
                if !snapshot.space_double_tap =>
            {
                self.app.last_space_press = Some(Instant::now());
            }
            (
                KeyCode::Char(' '),
                Some(Command::TogglePlayPause),
                RouterOutcome::Command(Command::TogglePlayPause),
            ) => self.app.last_space_press = None,
            (KeyCode::Esc, Some(Command::Stop), RouterOutcome::FallThrough)
                if !snapshot.esc_double_tap =>
            {
                self.app.last_esc_press = Some(Instant::now());
            }
            (KeyCode::Esc, Some(Command::Stop), RouterOutcome::Command(Command::Stop)) => {
                self.app.last_esc_press = None;
            }
            // any other (key, playback command, router outcome) triple: no
            // double-tap timer to arm or clear.
            _ => {}
        }
    }

    fn dispatch_router_command(&mut self, command: Command) -> bool {
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
    pub fn new(app: App) -> Self {
        Self::new_with_listener(
            app,
            EventListenerCfg::default()
                .crossterm_input_listener(TERMINAL_LISTENER_INTERVAL, TERMINAL_LISTENER_MAX_POLL),
        )
    }

    pub(in crate::app) fn new_with_listener(
        app: App,
        listener_cfg: EventListenerCfg<UserEvent>,
    ) -> Self {
        let application = Application::init(listener_cfg);
        let home_section = App::load_prefs()["home_section"]
            .as_str()
            .and_then(HomeLatestSource::from_pref_key);
        let mut model = Self {
            app,
            application,
            emby_browser_id: None,
            tv_workspace_id: None,
            music_workspace_id: None,
            abs_podcast_id: None,
            abs_book_id: None,
            mounted_destinations: std::collections::HashSet::new(),
            music_track_focus_request: None,
            music_workspace_reanchor: false,
            tv_workspace_reanchor: false,
            feeds_manage: None,
            home_content: HomeContent::new(),
            home_section_pref_semantic: home_section.clone(),
            home_section_pending: home_section,
        };
        // UiRoot owns overlay z-order and permanently observes terminal events.
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
        // Home is mounted for the whole session but never made active: its
        // input stays on the shell path, only its render is component-owned
        model.mount_home();
        model.mount_feeds();
        // Playback is also the stable attribute carrier for precedence gates.
        model
            .application
            .mount(
                ComponentId::Playback,
                Box::new(PlaybackComponent::new()),
                vec![],
            )
            .expect("mount Playback");
        model.update_settings_content();
        model
    }
}

fn apply_terminal_observer(
    model: &mut Model,
    event: TerminalObserverEvent,
    _focused: Option<&ComponentId>,
    music_resize: &mut bool,
    tv_resize: &mut bool,
    _quit: &mut bool,
) {
    match event {
        TerminalObserverEvent::Resize => {
            model.app.force_clear = true;
            model.app.card_image_states.clear();
            model.app.card_image_loading.clear();
            model.push_inline_search_content();
            *music_resize = true;
            *tv_resize = true;
        }
        TerminalObserverEvent::FocusGained => model.app.note_focus_gained(),
        TerminalObserverEvent::FocusLost => model.app.note_focus_lost(),
        TerminalObserverEvent::Key(_)
        | TerminalObserverEvent::Mouse
        | TerminalObserverEvent::NoOp => {}
    }
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
