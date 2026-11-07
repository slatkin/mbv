//! The GUI Client's routing entry (`extract-client-core` row 6.3): one call
//! that resolves translated neutral input through the shared keyboard router
//! (ADR 0023) and applies the outcome to the `App`.
//!
//! The snapshot's `App`-side facts — the player state, the panel mode and
//! focus, the prefix bit, and the idle-feed link — are `pub(in crate::app)`,
//! so the snapshot is built here in the library, not in a GUI program. The
//! GUI-side facts — the mounted surfaces and their Surface state — belong to
//! the GUI Client, which supplies the focused one as the router's leaf
//! ([`GuiSurfaceLeaf`]).
//!
//! The leaf arbitration mirrors the TUI Client's fold
//! (`shell::arbitration`): `Command` and an armed prefix dispatch run
//! immediately; `FallThrough` delivers the chord to the focused surface and
//! its message stands; `Deferred` delivers to the leaf first and fires only
//! when the surface did not consume the chord; `Swallow` runs nothing. The
//! messages a Surface state emits dispatch through
//! [`App::handle_gui_surface_msg`], whose Queue arms are the same App-side
//! effects the TUI shell's queue handlers run.

use mbv_surfaces::gesture::PointerGesture;
use mbv_ui_model::playback::QueueLink;
use mbv_ui_msg::{
    ComponentId, KeyEvent, LeafKeyResult, Msg, OverlayId, PointerEvent, Position, QueueIntent,
    QueueRequest, ShellRequest,
};

use super::key_policy::{OverlayFocus, RouterOverlayState, RouterPlaybackState};
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

/// The GUI Client's focused surface, seen from the shared router's leaf side.
/// The GUI owns its surfaces and their Surface state; the router sees one
/// focused leaf, exactly as the TUI Client's `TuiRealm` focus presents one.
pub trait GuiSurfaceLeaf {
    /// The focused leaf's stable id the router resolves against.
    fn id(&self) -> ComponentId;

    /// Whether the focused surface holds an active Visual selection: its
    /// row-local Space takes Space ahead of the global play/pause toggle
    /// (`media-list-multi-select`), so the router resolves the chord as a
    /// deferred candidate the surface can claim.
    fn visual_selection_active(&self) -> bool;

    /// Deliver one key chord to the focused Surface state. A consumed chord
    /// keeps the surface's message standing; an unhandled chord leaves the
    /// event to the router's deferred candidate.
    fn key(&mut self, key: KeyEvent) -> LeafKeyResult;

    /// Deliver one recognized pointer gesture to the focused Surface state.
    /// The surface resolves the gesture against its painted geometry and
    /// returns the message whose App-side effect the Client core dispatches;
    /// a gesture the surface's painted geometry does not claim returns
    /// `None`.
    fn pointer(&mut self, gesture: PointerGesture<Position>) -> Option<Msg>;
}

impl App {
    /// Route one GUI Client input through the shared keyboard router and
    /// apply the outcome to this `App` (row 6.3). `leaf` is the GUI's focused
    /// surface, or `None` when no surface holds the GUI's focus — then no
    /// leaf can consume anything, a `Deferred` candidate fires, and
    /// `Swallow`/`FallThrough` discard the event. Returns whether the `App`
    /// asked to quit (`AppCommand::Quit`), which the GUI Client answers by
    /// closing its window.
    ///
    /// A `Command::Shell` is shell-owned (`TuiRealm` mounts and the shell's
    /// component-local refresh flag), so no `App` handling exists for it and
    /// this entry leaves it undelivered: a surface the GUI Client does not
    /// yet present is absent (a GUI Client presents only the
    /// surfaces it has ported), and the GUI's own surface mounts own that arm as they land.
    pub fn route_gui_input(
        &mut self,
        input: GuiInput,
        leaf: Option<&mut dyn GuiSurfaceLeaf>,
    ) -> bool {
        let GuiInput::Key(key) = input else {
            // Any pointer event disarms prefix mode, exactly as every
            // terminal mouse event does in the TUI (design D6, task 6.1);
            // the gesture itself resolves below, against the focused leaf.
            self.prefix_armed = false;
            return false;
        };
        // A track popover left open when the local player went inactive has
        // no chip source: the frame already projects no overlay for it, so
        // the held state drops before the snapshot reads it — otherwise the
        // overlay focus would keep the global gates closed and the overlay's
        // key rules would swallow input the window does not paint.
        self.close_orphaned_track_popover();
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
                // The queue-only idle gate is the idle-feed link's own surface
                // concern; the first slice presents no idle-feed surface.
                queue_only_idle: false,
                idle_feed_link_available: self.idle_feed_link_available(),
            },
            // The GUI's one blocking overlay is the open row menu, track
            // popover, or playback-target chooser (add-tauri-gui-client rows
            // 6.2 and 6.4): the same focus fact the TUI's mounted context
            // menu reports, so the global bindings close while an overlay is
            // up and the overlay's own key rules consume the chord. No other
            // overlay mounts in this slice.
            overlays: RouterOverlayState {
                focus: if self.gui_overlay.is_some() {
                    OverlayFocus::ContextMenu
                } else {
                    OverlayFocus::Free
                },
                // The queue filter's text entry is the GUI's one text entry:
                // while it holds the keyboard the global bindings fall
                // through, so its characters type instead of firing actions
                // (add-tauri-gui-client row 6.2).
                text_entry_focused: self.queue_filter_active,
                ..RouterOverlayState::default()
            },
            panel_mode: self.effective_panel_mode(),
            panel_focus: self.effective_panel_focus(),
            prefix_armed: self.prefix_armed,
            visual_selection_active: leaf
                .as_ref()
                .is_some_and(|surface| surface.visual_selection_active()),
            // This entry is the GUI Client's: the GUI-only key context's
            // layers (add-tauri-gui-client row 6.2) route the canvas's `z`,
            // `q`, and `c` here and never in the TUI Client.
            gui_client: true,
        };
        // While the GUI's overlay is open it is the blocking overlay the
        // router arbitrates against: the chord's focused leaf is the overlay,
        // not the panel beneath it, so a FallThrough reaches the overlay's
        // key rules and not the queue surface's.
        let focused = if self.gui_overlay.is_some() {
            Some(ComponentId::Overlay(OverlayId::ContextMenu))
        } else {
            leaf.as_ref().map(|surface| surface.id())
        };
        match resolve_router_outcome_with_focused(key, &snapshot, focused.as_ref(), &keybinds) {
            RouterOutcome::Command(command) => self.apply_router_command(&command),
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
            RouterOutcome::Swallow => false,
            RouterOutcome::FallThrough => {
                // The open overlay owns the keyboard while it is up: the
                // router resolved no command, and the overlay — the GUI's
                // blocking overlay — consumes or drops the chord, never the
                // panel beneath it.
                if self.gui_overlay.is_some() {
                    self.apply_gui_overlay_key(key);
                    return false;
                }
                // The queue filter's text entry: its key rules run here, and
                // a chord they do not consume keeps falling through to the
                // focused surface, so navigation keeps working while the
                // filter is open.
                if self.apply_gui_filter_key(key) {
                    return false;
                }
                if let Some(surface) = leaf {
                    let result = surface.key(key);
                    self.handle_gui_surface_result(result);
                }
                false
            }
            RouterOutcome::Deferred(command) => {
                let consumed = match leaf {
                    Some(surface) => {
                        let result = surface.key(key);
                        let consumed = matches!(result, LeafKeyResult::Consumed(_));
                        self.handle_gui_surface_result(result);
                        consumed
                    }
                    // No focused leaf can consume the chord, so the deferred
                    // candidate fires, as it did before any surface mounted.
                    None => false,
                };
                if consumed {
                    false
                } else {
                    self.apply_router_command(&command)
                }
            }
        }
    }

    /// Route one recognized pointer gesture (row 4.8's shared policy, fed by
    /// the GUI's translation edge) to the focused surface and dispatch its
    /// App-side effect. Any pointer event disarms prefix mode first, exactly
    /// as every terminal mouse event does in the TUI. The gesture resolution
    /// runs against the surface's painted geometry from its last completed
    /// frame (`specs/client-interaction-core`).
    pub fn route_gui_pointer(
        &mut self,
        gesture: PointerGesture<Position>,
        leaf: Option<&mut dyn GuiSurfaceLeaf>,
    ) {
        self.prefix_armed = false;
        if let Some(surface) = leaf
            && let Some(msg) = surface.pointer(gesture)
        {
            self.handle_gui_surface_msg(msg);
        }
    }

    /// Dispatch one delivered leaf result: the message the focused surface
    /// emitted with a consumed chord. An unhandled chord carries no message.
    fn handle_gui_surface_result(&mut self, result: LeafKeyResult) {
        if let LeafKeyResult::Consumed(Some(msg)) = result {
            self.handle_gui_surface_msg(*msg);
        }
    }

    /// Dispatch one message a GUI surface emitted. The Queue Surface state
    /// emits `Queue` and `Shell` requests, and the Queue playback panel emits
    /// `Playback` requests, which run the same App-side effects the TUI shell
    /// runs. The `Service` and `TerminalEvent` variants belong to surfaces the
    /// first slice does not present, so their arms stay the documented no-ops.
    pub(crate) fn handle_gui_surface_msg(&mut self, msg: Msg) {
        match msg {
            Msg::Queue(request) => self.handle_gui_queue_request(request),
            Msg::Shell(request) => self.handle_gui_queue_shell_request(*request),
            Msg::Playback(request) => self.handle_playback_request(&request),
            Msg::Service(_) | Msg::TerminalEvent(_) => {}
        }
    }

    /// The `QueueRequest` dispatch for the GUI Client's Queue Surface state:
    /// the same App-side effects the TUI shell's `handle_queue_request` runs,
    /// minus the `TuiRealm`-owned refresh flags. Slot resolution and the
    /// link/focus/hold-window side effects mirror `select_queue_slot`.
    fn handle_gui_queue_request(&mut self, request: QueueRequest) {
        match request {
            QueueRequest::Cursor { link, slot_id } => {
                // Plain navigation: select_queue_slot applies the link/focus/
                // hold-window side effects; there is no effect to drive with
                // the resolved index (D2), so it is discarded.
                let _ = self.gui_select_queue_slot(link, slot_id);
            }
            QueueRequest::Play { link, slot_id } => {
                if let Some(index) = self.gui_select_queue_slot(link, slot_id) {
                    self.dispatch(&crate::app::dispatch::action::AppCommand::QueuePlayCursor(
                        index,
                    ));
                }
            }
            QueueRequest::Remove { link, slot_id } => {
                if let Some(index) = self.gui_select_queue_slot(link, slot_id) {
                    self.remove_from_queue(index);
                }
            }
            QueueRequest::RemoveSelection { link, slot_ids } => {
                // One batch edit: the owner applies the whole range before
                // publishing a queue snapshot, so the list does not shrink one
                // row per removal.
                self.remove_slots_from_queue(link, &slot_ids);
            }
            QueueRequest::Move {
                link,
                slot_id,
                direction,
            } => {
                if let Some(index) = self.gui_select_queue_slot(link, slot_id) {
                    match direction {
                        mbv_ui_msg::QueueMove::Up => self.move_queue_item_up(index),
                        mbv_ui_msg::QueueMove::Down => self.move_queue_item_down(index),
                    }
                }
            }
            QueueRequest::MoveTo {
                link,
                slot_id,
                onto,
            } => {
                let Some(from) = self.gui_select_queue_slot(link, slot_id) else {
                    return;
                };
                let Some(to) = self.slot_index(link, onto) else {
                    return;
                };
                if from != to {
                    self.move_queue_item_to(link, from, to);
                }
            }
            // The row drag's drop (add-tauri-gui-client row 6.5): the slot's
            // final queue index, so the move lands where the insertion line
            // was painted. One request per drag: the core emits it on the
            // release, and motion only previews the drop.
            QueueRequest::MoveToIndex {
                link,
                slot_id,
                to_index,
            } => {
                let Some(from) = self.gui_select_queue_slot(link, slot_id) else {
                    return;
                };
                if from != to_index {
                    self.move_queue_item_to(link, from, to_index);
                }
            }
            QueueRequest::ResizeColumnLive(_) | QueueRequest::ResizeColumnEnd(_) => {
                // The TUI-only queue-column resize surface has no GUI
                // counterpart; the GUI's arrangement is row 6.6's.
            }
            QueueRequest::Undo { link } => {
                // The surface can still show `Peer` for a frame after a
                // remote disconnect, before the projection refresh flips it
                // back; treat the undo as Local then, exactly as the TUI's
                // handler does. A peer-targeted undo has no GUI surface that
                // presents a remote-edit history, so it stays a no-op.
                if link == QueueLink::Peer && !self.has_direct_remote_queue() {
                    self.undo_last_queue_edit(QueueLink::Local);
                } else if link == QueueLink::Local {
                    self.undo_last_queue_edit(link);
                }
            }
        }
    }

    /// The `QueueIntent` dispatch for the GUI Client's Queue Surface state.
    /// The intents whose TUI effect opens a shell surface (the clear
    /// confirmation, the save-playlist dialog) or navigates a Library panel
    /// the GUI does not present stay the documented no-ops
    /// (a GUI Client presents only the surfaces it has ported).
    fn handle_gui_queue_intent(&mut self, intent: QueueIntent) {
        match intent {
            // The four intents whose TUI effect opens or reaches a surface
            // the GUI does not present — the clear confirmation modal, the
            // TUI's queue-column resize, the save-playlist dialog, and the
            // Library-panel navigation — stay undelivered
            // (a GUI Client presents only the surfaces it has ported).
            QueueIntent::Clear
            | QueueIntent::ResizeColumn(_)
            | QueueIntent::SavePlaylist
            | QueueIntent::Navigate { .. } => {}
            QueueIntent::PlayNow => {
                let (active, current_idx) = {
                    let status = self.player.status_snapshot();
                    (status.active, status.current_idx)
                };
                if active {
                    self.playback_queue_mut().set_cursor(current_idx);
                    // Jump-to-now-playing is an explicit, authoritative move.
                    self.pending_queue_cursor_reanchor = Some(self.playing_queue_link());
                }
                // The TUI flashes "Nothing is playing"; the toast surface is
                // row 6.5's, so the idle case stays silent until then.
            }
        }
    }

    /// The `ShellRequest` dispatch for the GUI Client's Queue Surface state.
    /// The Queue Surface state emits `QueueIntent`, `QueueRowClick`, and
    /// `QueueRowActivate` (the TUI's single and double click gestures), plus
    /// `RowContextMenu` and `SelectionProjection`. The remaining variants have
    /// no producer on this surface, and a request that reaches one of those
    /// arms would name a surface the Client does not present, so the arm is
    /// the same documented absence
    /// (a GUI Client presents only the surfaces it has ported).
    fn handle_gui_queue_shell_request(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::QueueIntent(intent) => self.handle_gui_queue_intent(intent),
            // A pointer single click on a row runs the TUI single click's App
            // effects: Queue panel focus, the queue hold window, and the
            // displayed cursor. The surface has already pinned its selection.
            ShellRequest::QueueRowClick { slot_id } => {
                let _ = self.handle_mouse_single_click_queue(slot_id);
            }
            // A pointer double click on a row runs the TUI double click: the
            // same single-click effects, then the play of the resolved row.
            ShellRequest::QueueRowActivate { slot_id } => {
                self.handle_mouse_double_click_queue(slot_id);
            }
            // Both open a surface the first slice does not present: the
            // context-menu overlay and the Status bar that shows the
            // selection summary (row 6.5). The requests stay undelivered
            // (a GUI Client presents only the surfaces it has ported).
            // A right-click on a row — or the keyboard's context-menu key,
            // which resolves through the same request — opens the GUI's row
            // menu (add-tauri-gui-client row 6.2): the Client-core overlay
            // the frame projects, whose entries carry the canvas's key hints.
            ShellRequest::RowContextMenu(targets, _) => match targets {
                mbv_ui_model::context_menu::ContextMenuTargets::Queue(slots) => {
                    self.open_gui_row_menu(&slots);
                }
                // The GUI's queue leaf resolves only Queue targets; the other
                // variants name rows of surfaces this Client does not present.
                mbv_ui_model::context_menu::ContextMenuTargets::Home(_)
                | mbv_ui_model::context_menu::ContextMenuTargets::Emby(_)
                | mbv_ui_model::context_menu::ContextMenuTargets::Feeds(_)
                | mbv_ui_model::context_menu::ContextMenuTargets::Audiobookshelf(_) => {}
            },
            // The open row menu's entry press (add-tauri-gui-client row 6.5):
            // the Client-core menu executes the entry the index names, the
            // same execution its Enter applies.
            ShellRequest::ContextMenuSelect(index) => self.select_gui_row_menu_entry(index),
            // The Status bar's selection summary is row 6.5's.
            ShellRequest::SelectionProjection(_) => {}
            // The header's playback-target press emits the same opener the
            // TUI's indicator sends the Sessions sidebar (add-tauri-gui-
            // client row 6.4); the GUI's counterpart is the Client-core
            // chooser overlay, and the press toggles it as F3 toggles the
            // sidebar.
            ShellRequest::OpenSessions => self.toggle_gui_target_chooser(),
            // A chooser row's press selects its output through the same
            // dispatch the TUI's Sessions sidebar applies on its selected
            // row: resolve the stable key against the current target
            // snapshot, do nothing when the target is gone, and close the
            // chooser on the selection the sidebar's dismiss carries.
            ShellRequest::SelectSession(key) => {
                if let Some(target) = crate::app::state::panel_targets::resolve_session_target(
                    &self.panel_targets,
                    &key,
                ) {
                    self.select_panel_target(target);
                    self.gui_overlay = None;
                }
            }
            request => {
                tracing::debug!(
                    name: "gui.surface_msg.undelivered",
                    target: "gui",
                    { request = ?request },
                    "a shell request no GUI surface emits reached the GUI dispatch"
                );
            }
        }
    }

    /// Resolve `slot_id` to its index in `link`'s queue and apply the
    /// link/focus/hold-window side effects, returning the resolved index —
    /// the GUI Client's `select_queue_slot`. The index is the operand for the
    /// shell-owned effect the caller is about to run (D2). The surface's own
    /// cursor is authoritative for selection; the App's queue cursor is not
    /// written here (task 3.1: the mirror is gone).
    fn gui_select_queue_slot(
        &mut self,
        link: QueueLink,
        slot_id: mbv_queue::QueueSlotId,
    ) -> Option<usize> {
        if link == QueueLink::Peer && !self.has_direct_remote_queue() {
            return None;
        }
        let index = self.slot_index(link, slot_id)?;
        self.set_panel_focus(mbv_ui_model::settings::PanelFocus::Queue);
        self.mark_queue_cursor_user_active();
        Some(index)
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

#[cfg(test)]
mod tests;
