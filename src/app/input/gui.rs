//! The GUI Client's routing entry (`add-gui-front-end` row 6.3): one call
//! that resolves translated neutral input through the shared keyboard router
//! (ADR 0023) and applies the outcome to the `App`.
//!
//! The snapshot's `App`-side facts — the player state, the panel mode and
//! focus, the prefix bit, and the idle-feed link — are `pub(in crate::app)`,
//! so the snapshot is built here in the library, not in `mbv-gui`. The
//! GUI-side facts — the mounted surfaces, the focus, and the text entry —
//! belong to the GUI Client's own state, and the first slice presents no
//! surfaces (rows 6.4 and 6.5 mount them), so they are the empty defaults
//! today; the GUI's `FocusStack` replaces them when it lands.

use mbv_ui_msg::{KeyEvent, PointerEvent};

use super::key_policy::{RouterOverlayState, RouterPlaybackState};
use super::router::{RouterOutcome, RouterSnapshot, resolve_router_outcome_with_focused};
use crate::app::dispatch::action::Command;
use crate::app::{App, RemoteControlTarget};

/// One translated GUI Client input event: the neutral shape the shared
/// router resolves, whatever toolkit produced it (design D4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuiInput {
    /// A key chord.
    Key(KeyEvent),
    /// A pointer event.
    Pointer(PointerEvent),
}

impl App {
    /// Route one GUI Client input through the shared keyboard router and
    /// apply the outcome to this `App` (row 6.3). Returns whether the `App`
    /// asked to quit (`AppCommand::Quit`), which the GUI Client answers by
    /// closing its window.
    ///
    /// The GUI Client has no focused leaf and no `TuiRealm`, so the router
    /// resolves with no focused component and the leaf-facing outcomes
    /// collapse: a `Deferred` candidate fires (no leaf can consume it), and
    /// `Swallow`/`FallThrough` discard the event. Prefix arming sets and
    /// clears the `App`-owned bit with no focus hold — the GUI's focus stack
    /// arrives with its surfaces.
    ///
    /// A `Command::Shell` is shell-owned (`TuiRealm` mounts and the shell's
    /// component-local refresh flag), so no `App` handling exists for it and
    /// this entry leaves it undelivered: a surface the GUI Client does not
    /// yet present is absent (`specs/gui-client`, "A surface is not yet
    /// ported"), and rows 6.4 and 6.5 own the GUI's mounts. Any pointer
    /// event disarms prefix mode, exactly as every terminal mouse event does
    /// in the TUI (design D6, task 6.1); gesture resolution against painted
    /// targets is rows 6.4 and 6.5.
    #[must_use]
    pub fn route_gui_input(&mut self, input: GuiInput) -> bool {
        let GuiInput::Key(key) = input else {
            self.prefix_armed = false;
            return false;
        };
        let keybinds = self
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keybinds
            .clone();
        // `player.status` is a plain (non-reentrant) mutex; read it in its
        // own statement, exactly as the TUI's arbitration does, so no
        // temporary inside the snapshot literal can hold the lock while the
        // snapshot's other initializers run.
        let player_active = self.player.status_snapshot().active;
        let snapshot = RouterSnapshot {
            playback: RouterPlaybackState {
                player_active,
                remote_target: RemoteControlTarget::DirectRemote,
                // The first GUI slice mounts no Queue playback panel (rows
                // 6.4 and 6.5 own it), so the queue-only idle gate stays shut.
                queue_only_idle: false,
                idle_feed_link_available: self.idle_feed_link_available(),
            },
            // The GUI presents no surfaces yet (rows 6.4 and 6.5): no overlay
            // is mounted, none holds focus, and no text entry exists.
            overlays: RouterOverlayState::default(),
            panel_mode: self.effective_panel_mode(),
            panel_focus: self.effective_panel_focus(),
            prefix_armed: self.prefix_armed,
            // The GUI has no Visual selection surface.
            visual_selection_active: false,
        };
        match resolve_router_outcome_with_focused(key, &snapshot, None, &keybinds) {
            // A `Command` runs now; a `Deferred` candidate fires too, because
            // no leaf can consume it in the GUI Client (`apply_deferred_
            // candidate`'s unhandled path).
            RouterOutcome::Command(command) | RouterOutcome::Deferred(command) => {
                self.apply_router_command(&command)
            }
            RouterOutcome::PrefixArm => {
                self.prefix_armed = true;
                false
            }
            RouterOutcome::PrefixDispatch(command) => {
                self.prefix_armed = false;
                self.apply_router_command(&command)
            }
            RouterOutcome::PrefixSwallow => {
                self.prefix_armed = false;
                false
            }
            RouterOutcome::Swallow | RouterOutcome::FallThrough => false,
        }
    }

    /// Apply one router `Command` to this `App`. The `App` arm is the App's
    /// own dispatch; the shell arm is the documented no-op
    /// [`route_gui_input`](Self::route_gui_input) describes.
    fn apply_router_command(&mut self, command: &Command) -> bool {
        match command {
            Command::App(app_command) => self.dispatch(app_command),
            Command::Shell(_) => false,
        }
    }
}
