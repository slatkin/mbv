//! The GUI Client's routing entry (`extract-client-core` row 6.3): one call
//! that resolves translated neutral input through the shared keyboard router
//! (ADR 0023) and applies the outcome to the `App`.
//!
//! The snapshot's `App`-side facts — the player state, the panel mode and
//! focus, the capabilities, and the idle-feed link — are `pub(in crate::app)`,
//! so the snapshot is built here in the library, not in a GUI program. The
//! prefix state the router reads outside the snapshot is this Client's own,
//! reached through the router's neutral [`RouterNow`] access (`GuiCore`'s
//! implementor) so the GUI routing entry never hands the router a field. The
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
//! [`GuiCore::handle_gui_surface_msg`], whose Queue arm runs the one
//! [`App::handle_queue_request`] the TUI shell delegates to as well.

use mbv_surfaces::gesture::PointerGesture;
use mbv_ui_msg::{
    ComponentId, KeyEvent, LeafKeyResult, Msg, OverlayId, PlaybackRequest, PointerEvent, Position,
    QueueIntent, QueueMove, ShellRequest,
};

use super::key_policy::{OverlayFocus, RouterOverlayState, RouterPlaybackState};
use super::resolver::KeyChord;
use super::router::{RouterOutcome, RouterSnapshot, resolve_router_outcome};
use crate::app::RemoteControlTarget;
use crate::app::dispatch::action::{
    AppCommand, Command, NavigationIntent, PanelIntent, PlaybackIntent,
};
use crate::gui::GuiCore;

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

    /// Emit the queue move request for the selection this surface paints,
    /// in `direction` (the GUI-only `Alt+J`/`Alt+K` actions, row 6.5). The
    /// leaf resolves the selected slot from its own Surface state — never
    /// the App's follow cursor — so the chord moves the row the user sees
    /// selected. A surface that paints no queue selection returns `None`.
    fn move_selected_row(&mut self, direction: QueueMove) -> Option<Msg>;
}

impl GuiCore {
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
        mut leaf: Option<&mut dyn GuiSurfaceLeaf>,
    ) -> bool {
        let GuiInput::Key(key) = input else {
            // Any pointer event disarms prefix mode, exactly as every
            // terminal mouse event does in the TUI (design D6, task 6.1);
            // the gesture itself resolves below, against the focused leaf.
            self.app.prefix_armed = false;
            return false;
        };
        // A track popover left open when the local player went inactive has
        // no chip source: the frame already projects no overlay for it, so
        // the held state drops before the snapshot reads it — otherwise the
        // overlay focus would keep the global gates closed and the overlay's
        // key rules would swallow input the window does not paint.
        self.close_orphaned_track_popover();
        // A new queue the local Owner made opens the Queue chooser on its name
        // here (design D8): the core made the queue and left the hand-off, and
        // the overlay is this Client's own presentation, so the draft shows
        // before this key resolves against the chooser it would paint. A
        // pointer pass sets no hand-off; the GUI core thread takes it after
        // the pass's dispatch instead.
        self.present_pending_gui_overlay();
        let keybinds = self
            .app
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keybinds
            .clone();
        // `player.status` is a plain (non-reentrant) mutex; read it in its
        // own statement, exactly as the TUI's arbitration does, so no
        // temporary inside the snapshot literal can hold the lock while the
        // snapshot's other initializers run.
        let player_active = self.app.shown_playback_status().active;
        let snapshot = RouterSnapshot {
            playback: RouterPlaybackState {
                player_active,
                remote_target: RemoteControlTarget::DirectRemote,
                // The queue-only idle gate is the idle-feed link's own surface
                // concern; the first slice presents no idle-feed surface.
                queue_only_idle: false,
                idle_feed_link_available: self.app.idle_feed_link_available(),
            },
            // The GUI's blocking overlays are the open row menu, track
            // popover, playback-target chooser, confirmation modal, and
            // daemon-lost dialog (add-tauri-gui-client rows 6.2 and 6.4, the
            // last two raised by `ask_confirm` and
            // `raise_daemon_lost_modal`): the same focus fact the TUI's
            // mounted context menu reports, so the global bindings close
            // while an overlay is up and the overlay's own key rules consume
            // the chord.
            overlays: RouterOverlayState {
                // The Settings screen is the non-blocking situation the
                // queue-scoped gates read closed (`add-gui-settings-screen`
                // design D2), while the global and transport actions fire.
                focus: if self.gui.overlay.is_some() {
                    OverlayFocus::ContextMenu
                } else if self.gui.settings_screen.is_some() {
                    OverlayFocus::SettingsScreen
                } else {
                    OverlayFocus::Free
                },
                // The queue filter's text entry is the GUI's one text entry:
                // while it holds the keyboard the global bindings fall
                // through, so its characters type instead of firing actions
                // (add-tauri-gui-client row 6.2).
                text_entry_focused: self.app.queue_filter_active,
                ..RouterOverlayState::default()
            },
            panel_mode: self.app.effective_panel_mode(),
            panel_focus: self.app.effective_panel_focus(),
            visual_selection_active: leaf
                .as_ref()
                .is_some_and(|surface| surface.visual_selection_active()),
            // The transport chips' key layers (add-tauri-gui-client row 6.2)
            // route the canvas's `z`, `q`, and `c` here — the GUI Client
            // holds `TransportChipKeys` — and never in the TUI Client.
            client_capabilities: self.app.client_capabilities,
        };
        // While the GUI's overlay is open it is the blocking overlay the
        // router arbitrates against: the chord's focused leaf is the overlay,
        // not the panel beneath it, so a FallThrough reaches the overlay's
        // key rules and not the queue surface's.
        let focused = if self.gui.overlay.is_some() {
            Some(ComponentId::Overlay(OverlayId::ContextMenu))
        } else {
            leaf.as_ref().map(|surface| surface.id())
        };
        // The GUI bridges its toolkit's keys to the neutral vocabulary itself
        // (`mbv-gui`'s `bridge/keys.rs`, mirroring the TUI edge); the router
        // normalizes the chord and resolves it. No translation happens here.
        let chord = KeyChord::from_key(key);
        // The router's one entry (design D6, row 8.4): the snapshot carries
        // the policy's facts and `self` carries the Client's own neutral state
        // through `RouterNow`, so this site reads no routing state of its own
        // and resolves through the same rules the TUI Client does.
        match resolve_router_outcome(chord, &snapshot, self, focused.as_ref(), &keybinds) {
            RouterOutcome::Command(command) => self.apply_gui_router_command(&command, leaf),
            RouterOutcome::PrefixArm => {
                self.app.prefix_armed = true;
                false
            }
            RouterOutcome::PrefixDispatch(command) => {
                self.app.prefix_armed = false;
                self.apply_gui_router_command(&command, leaf)
            }
            RouterOutcome::PrefixSwallow => {
                self.app.prefix_armed = false;
                false
            }
            RouterOutcome::Swallow => false,
            RouterOutcome::FallThrough => {
                // The open overlay owns the keyboard while it is up: the
                // router resolved no command, and the overlay — the GUI's
                // blocking overlay — consumes or drops the chord, never the
                // panel beneath it. The daemon-lost dialog's quit choice
                // returns the Client's quit decision, which ends the run
                // exactly as a routed quit key does.
                if self.gui.overlay.is_some() {
                    return self.apply_gui_overlay_key(key);
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
                let consumed = match leaf.as_mut() {
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
                    self.apply_gui_router_command(&command, leaf)
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
        self.app.prefix_armed = false;
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
            Msg::Queue(request) => self.app.handle_queue_request(request),
            Msg::Shell(request) => self.handle_gui_queue_shell_request(*request),
            Msg::Playback(request) => self.handle_gui_playback_request(&request),
            Msg::Service(_) | Msg::TerminalEvent(_) => {}
        }
    }

    /// The GUI Client's playback request: the shared App-side effect first
    /// (`App::handle_playback_request`), then the Client's own half of the
    /// requests whose effect is GUI presentation. Those requests are the
    /// documented core no-ops of design D4a (a) — the track chip's popover
    /// open, the track popover's row press, the player menu's open and entry
    /// press, and the Queue chooser's entry press — so each pair runs the core
    /// and then the GUI method here, in the order the core ran them before the
    /// split. The requests whose whole effect is GUI presentation and whose
    /// core half is a panel intent (the header's collapse, queue-fold, and
    /// chooser controls) are listed here too: a header control's press reaches
    /// only this entry, so without the GUI method the control would do nothing
    /// while its chord still worked.
    fn handle_gui_playback_request(&mut self, request: &PlaybackRequest) {
        self.app.handle_playback_request(request);
        match request {
            PlaybackRequest::TrackPopoverSelect(track_id) => {
                self.select_gui_track_popover_track(*track_id);
            }
            PlaybackRequest::TogglePanelCollapse => self.toggle_panel_collapse(),
            PlaybackRequest::ToggleQueueFold => self.toggle_queue_fold(),
            PlaybackRequest::OpenPlayerMenu { x, y } => self.open_gui_player_menu(*x, *y),
            PlaybackRequest::PlayerMenuSelect => self.select_gui_player_menu_entry(),
            PlaybackRequest::QueueChooserEntry(queue) => {
                self.show_gui_queue_chooser_entry(*queue);
            }
            PlaybackRequest::OpenTrackPopover(chip) => self.toggle_gui_track_popover(*chip),
            // The canvas queue header's chooser control (design D8): the
            // request's core arm dispatches `PanelIntent::ToggleQueueChooser`,
            // which is a documented core no-op, so the toggle runs here. The
            // `t` chord reaches the same method through `apply_gui_command`.
            PlaybackRequest::QueueChooserToggle => self.toggle_gui_queue_chooser(),
            // The transport controls run their whole effect in the core, and
            // the request's own arm already applied it above. The remaining
            // queue-header controls dispatch a panel intent the core applies
            // itself (the filter's chord equivalent, the new, rename, and
            // close requests), so their effect is complete before this match.
            PlaybackRequest::TogglePlayPause
            | PlaybackRequest::Stop
            | PlaybackRequest::Previous
            | PlaybackRequest::Next
            | PlaybackRequest::SeekTo(_)
            | PlaybackRequest::SeekRelative(_)
            | PlaybackRequest::ToggleMute
            | PlaybackRequest::VolumeDelta(_)
            | PlaybackRequest::CycleAudio
            | PlaybackRequest::CycleSubtitle
            | PlaybackRequest::ActivateQueueFilter
            | PlaybackRequest::NewQueue
            | PlaybackRequest::RenameQueue
            | PlaybackRequest::CloseQueue => {}
        }
    }

    /// The `QueueIntent` dispatch for the GUI Client's Queue Surface state.
    /// The intents whose TUI effect opens a shell surface (the clear
    /// confirmation, the save-playlist dialog) or navigates a Library panel
    /// the GUI does not present stay the documented no-ops
    /// (a GUI Client presents only the surfaces it has ported).
    fn handle_gui_queue_intent(&mut self, intent: QueueIntent) {
        match intent {
            // The clear request runs the TUI's own App action: the prompt the
            // `c` chord raises, which the GUI now presents as its confirm
            // overlay (the TUI's mounted Confirm modal). The three intents
            // whose TUI effect opens or reaches a surface the GUI does not
            // present — the TUI's queue-column resize, the save-playlist
            // dialog, and the Library-panel navigation — stay undelivered
            // (a GUI Client presents only the surfaces it has ported).
            QueueIntent::Clear => self.app.request_clear_queue(),
            QueueIntent::ResizeColumn(_)
            | QueueIntent::SavePlaylist
            | QueueIntent::Navigate { .. } => {}
            QueueIntent::PlayNow => {
                let (active, current_idx) = {
                    let status = self.app.shown_playback_status();
                    (status.active, status.current_idx)
                };
                if active {
                    self.app.playback_queue_mut().set_cursor(current_idx);
                    // Jump-to-now-playing is an explicit, authoritative move.
                    self.app.pending_queue_cursor_reanchor = Some(self.app.playing_queue_link());
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
                let _ = self.app.handle_mouse_single_click_queue(slot_id);
            }
            // A pointer double click on a row runs the TUI double click: the
            // same single-click effects, then the play of the resolved row.
            ShellRequest::QueueRowActivate { slot_id } => {
                self.app.handle_mouse_double_click_queue(slot_id);
            }
            // Both open a surface the first slice does not present: the
            // context-menu overlay and the Status bar that shows the
            // selection summary (row 6.5). The requests stay undelivered
            // (a GUI Client presents only the surfaces it has ported).
            // A right-click on a row — or the keyboard's context-menu key,
            // which resolves through the same request — opens the GUI's row
            // menu (add-tauri-gui-client row 6.2): the Client-core overlay
            // the frame projects, whose entries carry the canvas's key hints.
            ShellRequest::RowContextMenu(targets) => match targets {
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
                if self.select_target_for_key(&key) {
                    self.gui.overlay = None;
                }
            }
            ShellRequest::SelectChooserTarget(key) => {
                if self.select_chooser_target(&key) {
                    self.gui.overlay = None;
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

    /// Apply one router `Command` to this `App`, except the queue-move
    /// command, which the focused leaf resolves against its own painted
    /// selection (add-tauri-gui-client row 6.5): the chord must move the row
    /// the user sees selected, not the App's follow cursor. The leaf emits
    /// the same `QueueRequest::Move` the `Shift+Up`/`Shift+Down` key rule
    /// emits, and that request's dispatch resolves the slot's index. Without
    /// a focused leaf the chord moves nothing.
    ///
    /// The `App` arm calls the core first and then applies the Client's own
    /// half of the panel actions whose effect is GUI presentation (design D4a
    /// (a)): the collapse and queue-fold toggles, the queue chooser, the
    /// playback-target chooser, and the track popover's toggle. Each is a
    /// documented core no-op, so the pair runs the core and then the GUI
    /// method, as the core ran it before the split. The `Shell` arm stays the
    /// documented no-op [`route_gui_input`](Self::route_gui_input) describes.
    fn apply_gui_router_command(
        &mut self,
        command: &Command,
        leaf: Option<&mut dyn GuiSurfaceLeaf>,
    ) -> bool {
        // The destination steps (ADR 0040, design D2 of
        // `gui-pointer-target-interaction`): the window content takes neither
        // Tab nor Shift+Tab, so they arrive here as the core's tab
        // navigation, and a digit arrives as a numbered destination. Each
        // opens through the window's one open step, so a step acts as a press
        // on the tab the top bar paints and never opens a destination the GUI
        // does not present. The other navigation intents keep their own
        // dispatch.
        if let Command::App(AppCommand::Navigation(intent)) = command {
            match intent {
                NavigationIntent::NextLibraryTab => {
                    self.step_gui_library_tab(1);
                    return false;
                }
                NavigationIntent::PreviousLibraryTab => {
                    self.step_gui_library_tab(-1);
                    return false;
                }
                NavigationIntent::SetLibraryTab(position) => {
                    self.open_gui_library_position(*position);
                    return false;
                }
                // Not destination steps: each of these keeps its own dispatch
                // below, as it does in the TUI Client.
                NavigationIntent::ForceClear
                | NavigationIntent::RequestClearQueue
                | NavigationIntent::ToggleSettings
                | NavigationIntent::OpenSessions
                | NavigationIntent::OpenPlaylists
                | NavigationIntent::OpenSearch => {}
            }
        }
        if let Command::App(AppCommand::Panel(PanelIntent::MoveQueueRow(direction))) = command {
            if let Some(surface) = leaf
                && let Some(msg) = surface.move_selected_row(*direction)
            {
                self.handle_gui_surface_msg(msg);
            }
            return false;
        }
        // The Panel-focus chord is a documented no-op in the GUI Client
        // (design D3 of `gui-pointer-target-interaction`): the GUI has no
        // Panel focus, so `panel_right` and `panel_left` are TUI-only in the
        // keybind registry and the core sets the stack's Panel focus per key
        // from the key's target instead. A stale binding that still resolves
        // here changes nothing.
        if matches!(
            command,
            Command::App(AppCommand::Panel(PanelIntent::FocusPanel(_)))
        ) {
            return false;
        }
        self.apply_router_command(command)
    }

    /// The GUI Client's own half of one `AppCommand`, applied after the core
    /// (design D4a (a)): the panel actions whose effect is GUI presentation,
    /// and the track popover's toggle. Each of their core arms is a documented
    /// no-op, so this runs the GUI method the core called before the split.
    ///
    /// Every variant is listed, as `AGENTS.md` requires: a wildcard here would
    /// drop a new `PanelIntent` that needs a GUI half without a word, and the
    /// arm list is the one place that pairing is visible.
    fn apply_gui_command(&mut self, command: &AppCommand) {
        match command {
            AppCommand::Panel(PanelIntent::TogglePanelCollapse) => self.toggle_panel_collapse(),
            AppCommand::Panel(PanelIntent::ToggleQueueFold) => self.toggle_queue_fold(),
            AppCommand::Panel(PanelIntent::ToggleQueueChooser) => self.toggle_gui_queue_chooser(),
            AppCommand::Panel(PanelIntent::ToggleTargetChooser) => {
                self.toggle_gui_target_chooser();
            }
            AppCommand::Playback(PlaybackIntent::ToggleTrackPopover(chip)) => {
                self.toggle_gui_track_popover(*chip);
            }
            // The commands with no GUI half. The panel intents the core applies
            // itself, or whose GUI half is an entry of its own: the layout cycle
            // and the panel focus (core only), the queue filter (the core runs
            // it), the new, close, and rename requests (one core path each), and
            // the row move (the router hands it to the focused Queue leaf). The
            // playback intents: each is a transport control whose whole effect
            // is in the core, or the chip opener, whose GUI half is the
            // `ToggleTrackPopover` arm above. The rest name no GUI presentation.
            AppCommand::Panel(
                PanelIntent::CyclePanelMode
                | PanelIntent::FocusPanel(_)
                | PanelIntent::ActivateQueueFilter
                | PanelIntent::RequestNewQueue
                | PanelIntent::RequestCloseQueue
                | PanelIntent::RequestRenameQueue
                | PanelIntent::MoveQueueRow(_),
            )
            | AppCommand::Playback(
                PlaybackIntent::TogglePlayPause
                | PlaybackIntent::Stop
                | PlaybackIntent::SeekRelative(_)
                | PlaybackIntent::NextTrack
                | PlaybackIntent::PreviousTrack
                | PlaybackIntent::CycleOrToggleSubtitle
                | PlaybackIntent::AdjustVolume(_)
                | PlaybackIntent::ToggleMute
                | PlaybackIntent::CycleAudio,
            )
            | AppCommand::Navigation(_)
            | AppCommand::Visual(_)
            | AppCommand::QueuePlayCursor(_)
            | AppCommand::Quit => {}
        }
    }

    /// Apply one router `Command` to this core. The `App` arm runs the core's
    /// own dispatch on the owned `App` first, then this Client's own half of
    /// it (design D4a (a)); the shell arm is the documented no-op
    /// [`route_gui_input`](Self::route_gui_input) describes.
    fn apply_router_command(&mut self, command: &Command) -> bool {
        match command {
            Command::App(app_command) => {
                let quit = self.app.dispatch(app_command);
                self.apply_gui_command(app_command);
                quit
            }
            Command::Shell(_) => false,
        }
    }
}

#[cfg(test)]
mod tests;
