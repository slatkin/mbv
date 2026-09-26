use super::components::{ComponentId, Msg, OverlayId, TerminalObserverEvent};
use super::{Model, DOUBLE_ESC_STOP_WINDOW};
use crate::app::dispatch::action::Command;
use crate::app::input::router::{
    resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot,
};

/// The ADR 0023 Keyboard Router fold: apply the router's outcome to this
/// tick's message list and return the messages that survive.
///
/// `Application::tick` returns the focused component's message first, then the
/// `UiRoot` observer's `TerminalEvent`. With `PollStrategy::Once` there is at
/// most one terminal event per tick, so the messages for a key chord are:
///
/// * **`UiRoot` focused** — only the observer's `TerminalEvent(Key)`. This is
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
) -> (Vec<Msg>, super::ArbitrationDiagnostic) {
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
        let key = crate::app::input::resolver::tuirealm_key_to_crossterm(tui_key);
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
            playback: crate::app::input::RouterPlaybackState {
                player_active,
                remote_target: if self.app.connected_session_id.is_some() {
                    crate::app::input::RemotePlaybackTarget::Session
                } else if self.app.player.is_remote() {
                    crate::app::input::RemotePlaybackTarget::DirectRemote
                } else if self.app.is_cast_attached() {
                    crate::app::input::RemotePlaybackTarget::Cast
                } else {
                    crate::app::input::RemotePlaybackTarget::None
                },
                queue_only_idle: self.application.mounted(&ComponentId::QueuePlaybackPanel)
                    && !self.app.effective_playback_state().active,
                idle_feed_link_available: self.app.idle_feed_link_available(),
            },
            overlays: crate::app::input::RouterOverlayState {
                focus: if self
                    .application
                    .mounted(&ComponentId::Overlay(OverlayId::ContextMenu))
                {
                    crate::app::input::OverlayFocus::ContextMenu
                } else if self.blocking_overlay_active() {
                    crate::app::input::OverlayFocus::Blocking
                } else if self.overlay_holds_focus() {
                    crate::app::input::OverlayFocus::NonBlocking
                } else {
                    crate::app::input::OverlayFocus::Free
                },
                help: self
                    .application
                    .mounted(&ComponentId::Overlay(OverlayId::Help)),
                sessions_sidebar: self
                    .application
                    .mounted(&ComponentId::Overlay(OverlayId::Sessions)),
                text_entry_focused: matches!(
                    self.application.focus(),
                    Some(ComponentId::Overlay(
                        OverlayId::Search | OverlayId::Settings
                    ))
                ) || self.active_inline_search_is_open(),
            },
            panel_mode: self.app.effective_panel_mode(),
            panel_focus: self.app.effective_panel_focus(),
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
        let quit = self.dispatch_router_command(command);
        debug_assert!(!quit, "deferred candidates never quit");
        true
    }

    pub(in crate::app) fn dispatch_router_command(&mut self, command: &Command) -> bool {
        match command {
            Command::OpenHelp => {
                self.mount_help();
                false
            }
            _ => self.app.dispatch(command),
        }
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
