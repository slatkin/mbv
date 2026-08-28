//! Shell `Model` — the migration's mixed-framework home (design D2/D11/D13).
//!
//! The `Model` owns the legacy `App` (it draws the current UI and runs the
//! existing handlers directly) and the TuiRealm
//! `Application<ComponentId, Msg, UserEvent>`. `Model::run` is the moved body
//! of the former `App::run`: the event-poll section is replaced by
//! `application.tick(PollStrategy::Once(..))`, whose messages are folded back
//! into the existing `App` input handlers via typed shell requests. Every
//! other part of the loop (receiver drains, periodic work,
//! render cadence via `wants_terminal_render`, teardown) is byte-for-byte the
//! legacy behaviour, only with `self.x` rewritten to `self.app.x`.
//!
//! TODO(migrate-tui-to-tuirealm): this whole module is the strangler-phase
//! shell. It shrinks as surfaces convert and is gone at the completion gate
//! (task 5.3/5.6), leaving only shell/runtime authority + the `Application`.

use std::time::Duration;

use tuirealm::application::{Application, PollStrategy};
use tuirealm::listener::EventListenerCfg;

use super::components::msg::{
    AlbumCursorKind, BrowserHitRegion, PodcastShowMove, QueueHitRegion, TvHitRegion,
};
use super::components::{
    ComponentId, Msg, OverlayId, PlaybackComponent, ShellRequest, TerminalObserverEvent,
    UiRootComponent, UserEvent,
};
use super::router::{resolve_router_outcome, RouterOutcome, RouterSnapshot};
use super::service_startup;
use super::types_feeds_manage::FeedsManagePopup;
use super::types_playback::{HomeContent, HomeLatestSource};
use super::{
    init_terminal, install_signal_handlers, restore_terminal, start_quit_watchdog, QUIT_REQUESTED,
};
use super::{App, IdleFeed, QueueScope, ToastSeverity};

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
    /// One-shot shell→component request for the mounted Music workspace's
    /// inline track focus, applied at the next `sync_music_workspace` after
    /// the component is mounted/synced (so mount-timing never loses it).
    /// `Some(true)` = enter focus (recursive album activation);
    /// `Some(false)` = clear focus (position restore). Neither mirrors App
    /// state: the component owns the cursor, the shell only delivers the
    /// trigger that used to write the deleted inline track-focus field.
    pub(super) music_track_focus_request: Option<bool>,
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
///   the active component's own message; `FallThrough` keeps it (so
///   `handle_legacy_key` runs, preserving today's behavior), while
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
                // own message below. It never reaches the legacy handler as a
                // raw key.
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
    fn router_outcome(&self, messages: &[Msg]) -> RouterOutcome {
        let Some(key) = messages.iter().find_map(|msg| match msg {
            Msg::TerminalEvent(TerminalObserverEvent::Key(key)) => Some(*key),
            _ => None,
        }) else {
            return RouterOutcome::FallThrough;
        };

        let snapshot = RouterSnapshot {
            player_active: self.app.player.status.lock().unwrap().active,
            has_remote_session: self.app.connected_session_id.is_some()
                || self.app.player.is_remote()
                || self.app.is_cast_attached(),
            panel_mode: self.app.effective_panel_mode(),
            blocking_overlay_open: self.is_blocking_overlay_open(),
            selection_modal_open: self
                .application
                .mounted(&ComponentId::Overlay(OverlayId::SelectionModal)),
            context_menu_open: self
                .application
                .mounted(&ComponentId::Overlay(OverlayId::ContextMenu)),
            idle_feed_link_available: self.app.idle_feed_link_available(),
        };
        resolve_router_outcome(key, &snapshot)
    }
}

fn apply_terminal_observer(
    model: &mut Model,
    event: TerminalObserverEvent,
    focused: Option<&ComponentId>,
    music_resize: &mut bool,
    tv_resize: &mut bool,
    quit: &mut bool,
) {
    match event {
        TerminalObserverEvent::Key(key) if focused == Some(&ComponentId::UiRoot) => {
            *quit |= model.handle_legacy_key(key);
        }
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

impl Model {
    /// Handle a key that remains on the legacy App path.
    fn handle_legacy_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        // F1 opens the Help overlay unless a blocking overlay is active (those
        // swallow it). Once Help is mounted it is the active component, so
        // F1 arrives as Msg::Shell(DismissHelp) instead.
        let quit = if key.code == crossterm::event::KeyCode::F(1)
            && !self
                .application
                .mounted(&ComponentId::Overlay(OverlayId::Help))
            && !self.is_blocking_overlay_open()
        {
            self.mount_help();
            false
        } else {
            self.app.handle_key_with_home_context(
                key,
                self.home_continue_watching_selected(),
                self.home_cw_item(),
            )
        };
        // F5/context-menu/confirm keys and panel-focus keys write Home
        // content or focus inside App's handler; re-project after every key
        // at this seam (idempotent) (task 5.3d, sync_home deletion).
        self.push_home_content();
        // Emby browser content may have changed (5.3d.15/M2).
        self.push_emby_browser_content();
        // Podcast keys (cursor/selection/filter moves and panel-focus keys)
        // write the active ABS browse state in App's handler; re-project
        // (5.3d.11 U6).
        self.push_audiobookshelf_podcast_content();
        // Book keys (cursor/selection/bucket moves and panel-focus keys) write
        // the active ABS browse state in App's handler; re-project (task 5.3d).
        self.push_audiobookshelf_book_content();
        self.push_music_workspace_content();
        quit
    }

    /// Construct the model, starting the TuiRealm crossterm listener and
    /// mounting the permanent root observer.
    pub fn new(app: App) -> Self {
        let listener_cfg = EventListenerCfg::default()
            .crossterm_input_listener(TERMINAL_LISTENER_INTERVAL, TERMINAL_LISTENER_MAX_POLL);
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
            music_track_focus_request: None,
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
        // (task 3.4; see `shell_home.rs`/`components::home`'s module docs).
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

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
