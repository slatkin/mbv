use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation, RowIntent,
    RowLocalInput, RowLocalOutcome,
};
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use super::msg::{
    Msg, QueueColumnResize, QueueIntent, QueueMove, QueueRequest, ShellRequest,
    TerminalObserverEvent,
};
use super::user_event::UserEvent;
use crate::app::palette;
use crate::app::render::{
    render_queue_body, render_queue_title_content, QueuePresentation, QueueRenderGeometry,
    QueueTitleModel,
};
use crate::app::types_playback::{PlaybackState, QueueScope};
use crate::app::ui_util::{fmt_duration_short, fmt_playback_pct};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::{QueueItem, QueueSlot, QueueSlotId};

/// Why the shell is pushing a cursor. `Preserve` keeps the user's selection
/// pinned to its slot across a content refresh; `Set` is an authoritative
/// move (follow-the-playhead, jump-to-now-playing, wheel scroll, scope switch)
/// that must win over slot-identity reconciliation.
pub(in crate::app) enum QueueCursorUpdate {
    Preserve,
    Set(usize),
}

pub struct QueueComponent {
    /// The one shared canonical owner of the Queue rows, carried by the Wide
    /// presentation in every panel mode (design.md D1/D2). It owns the local
    /// cursor, the resting scroll offset, and viewport/scrollbar geometry; the
    /// parent keeps only the prepared projection and shell-owned chrome below.
    carrier: MediaListCarrier<QueueSlotId>,
    scope: QueueScope,
    focused: bool,
    empty_text: String,
    title: Option<QueueTitleModel>,
    title_area: Option<Rect>,
    area: Rect,
    geometry: QueueRenderGeometry,
    pending_slot: Option<QueueSlotId>,
    drag_grab: Option<QueueSlotId>,
    throbber: Option<char>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle.
    mouse_gestures: MouseGestureState,
    /// Scope-pill rects (design.md D6), repopulated in `view()` from the
    /// geometry the title painter just produced.
    scope_regions: HitRegions<QueueScope>,
}

impl QueueComponent {
    pub(crate) fn selected_row_rect(&self) -> Option<Rect> {
        self.carrier.wide().current_selected_row_rect()
    }

    pub fn new() -> Self {
        Self {
            carrier: MediaListCarrier::new(Presentation::Wide),
            scope: QueueScope::Local,
            focused: false,
            empty_text: String::new(),
            title: None,
            title_area: None,
            area: Rect::default(),
            geometry: QueueRenderGeometry::default(),
            pending_slot: None,
            drag_grab: None,
            throbber: None,
            mouse_gestures: MouseGestureState::new(),
            scope_regions: HitRegions::new(),
        }
    }

    /// Test-only: drive framework focus the way `Component::attr` does.
    #[cfg(test)]
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn set_content(
        &mut self,
        slots: Vec<QueueSlot>,
        cursor: QueueCursorUpdate,
        scope: QueueScope,
        playback: PlaybackState,
        title: QueueTitleModel,
    ) {
        self.set_rows(slots, playback);
        self.set_cursor(cursor);
        self.set_scope_chrome(scope, title);
    }

    /// Replace projected rows while preserving the canonical list's selection.
    pub(in crate::app) fn set_rows(&mut self, slots: Vec<QueueSlot>, playback: PlaybackState) {
        self.carrier
            .set_content(queue_media_rows(&slots, playback, self.pending_slot));
    }

    /// Patch one projected row by stable target without rebuilding the list.
    pub(in crate::app) fn set_row_patch(
        &mut self,
        target: &QueueSlotId,
        row: MediaListRow<QueueSlotId>,
    ) -> bool {
        self.carrier.patch_row(target, row)
    }

    /// Deliver an authoritative cursor command independently of row delivery.
    /// This is the adjudicated Queue shell-push seam (design.md D5): the shell
    /// owns the Queue cursor command, so this one numeric re-anchor and its
    /// resting-scroll clamp are sanctioned rather than delegated.
    pub(in crate::app) fn set_cursor(&mut self, cursor: QueueCursorUpdate) {
        if let QueueCursorUpdate::Set(idx) = cursor {
            self.carrier.select_index(idx);
        }
        let scroll = self.carrier.scroll();
        let clamped = scroll.min(self.carrier.cursor());
        if clamped != scroll {
            self.carrier.set_scroll(clamped);
        }
    }

    /// Deliver the current scope and title/chrome independently of row delivery.
    pub(in crate::app) fn set_scope_chrome(&mut self, scope: QueueScope, title: QueueTitleModel) {
        if scope != self.scope {
            self.carrier.set_scroll(0);
        }
        self.scope = scope;
        self.empty_text = if scope == QueueScope::Local {
            "  Add items with p from Home or library tabs".into()
        } else {
            "  Remote queue is empty".into()
        };
        self.title = Some(title);
    }

    pub(in crate::app) fn set_pending_slot(&mut self, slot: Option<QueueSlotId>) {
        self.pending_slot = slot;
    }

    pub(in crate::app) fn set_throbber(&mut self, throbber: Option<char>) {
        self.throbber = throbber;
    }

    pub(in crate::app) fn set_area(&mut self, area: Rect) {
        self.area = area;
    }

    pub(in crate::app) fn set_title_area(&mut self, area: Option<Rect>) {
        self.title_area = area;
    }

    fn cursor_message(&self) -> Option<Msg> {
        self.carrier.selected_target().map(|&slot_id| {
            Msg::Queue(QueueRequest::Cursor {
                scope: self.scope,
                slot_id,
            })
        })
    }

    /// The one seam through which Queue offers an already-normalized row-local
    /// key or pointer gesture to the shared owner. The owner applies the local
    /// state transition and returns the closed provider-neutral outcome; Queue
    /// translates external row intents into its typed Msgs (design.md D3).
    fn delegate_row_local_input(
        &mut self,
        input: RowLocalInput,
        pointer_target: Option<QueueSlotId>,
    ) -> RowLocalOutcome<QueueSlotId> {
        self.carrier.delegate(input, pointer_target)
    }

    /// The scope/slot pair for Queue's current selection.
    fn selected_slot(&self) -> Option<(QueueScope, QueueSlotId)> {
        self.carrier
            .selected_target()
            .map(|&slot_id| (self.scope, slot_id))
    }

    /// Move the shared owner into the active presentation when they diverge.
    /// Queue keeps the Wide presentation in every panel mode (spec), so this
    /// preserves the fixed-row contract while keeping the owner behind the
    /// carrier's presentation seam.
    fn ensure_carrier(&mut self) {
        self.carrier
            .ensure_presentation(Presentation::Wide, self.area.height.max(1) as usize);
    }

    fn move_cursor(&mut self, delta: i64) -> Option<Msg> {
        match self.delegate_row_local_input(RowLocalInput::Move(delta), None) {
            RowLocalOutcome::SelectedTargetChanged(_) => self.cursor_message(),
            _ => None,
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Char('[')
                if !key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
            {
                self.scope = QueueScope::Local;
                // Scope is preassigned here, before the request reaches the
                // shell, so the set_content scope-change reset would not fire;
                // the component resets its own scroll itself (D3).
                self.carrier.set_scroll(0);
                return Some(Msg::Queue(QueueRequest::Scope(self.scope)));
            }
            Key::Char(']')
                if !key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
            {
                self.scope = QueueScope::Remote;
                // Scope is preassigned here, before the request reaches the
                // shell, so the set_content scope-change reset would not fire;
                // the component resets its own scroll itself (D3).
                self.carrier.set_scroll(0);
                return Some(Msg::Queue(QueueRequest::Scope(self.scope)));
            }
            Key::Left | Key::Right if key.modifiers == tuirealm::event::KeyModifiers::SHIFT => {
                return Some(Msg::Shell(ShellRequest::QueueIntent(
                    QueueIntent::ResizeColumn(if key.code == Key::Left {
                        QueueColumnResize::Narrower
                    } else {
                        QueueColumnResize::Wider
                    }),
                )));
            }
            Key::Up if key.modifiers.is_empty() => {
                return self.move_cursor(-1);
            }
            Key::Down if key.modifiers.is_empty() => {
                return self.move_cursor(1);
            }
            Key::PageUp if key.modifiers.is_empty() => {
                return self.move_cursor(-(self.area.height.saturating_sub(1).max(1) as i64));
            }
            Key::PageDown if key.modifiers.is_empty() => {
                return self.move_cursor(self.area.height.saturating_sub(1).max(1) as i64);
            }
            Key::Home if key.modifiers.is_empty() => {
                self.delegate_row_local_input(RowLocalInput::First, None);
                return self.cursor_message();
            }
            Key::End if key.modifiers.is_empty() => {
                self.delegate_row_local_input(RowLocalInput::Last, None);
                return self.cursor_message();
            }
            Key::Enter => {
                return match self.delegate_row_local_input(RowLocalInput::Activate, None) {
                    RowLocalOutcome::External(RowIntent::Activate(slot_id)) => {
                        Some(Msg::Queue(QueueRequest::Play {
                            scope: self.scope,
                            slot_id,
                        }))
                    }
                    _ => None,
                };
            }
            Key::Delete => {
                return self
                    .selected_slot()
                    .map(|(scope, slot_id)| Msg::Queue(QueueRequest::Remove { scope, slot_id }));
            }
            Key::Up if key.modifiers.contains(tuirealm::event::KeyModifiers::SHIFT) => {
                return self.selected_slot().map(|(scope, slot_id)| {
                    Msg::Queue(QueueRequest::Move {
                        scope,
                        slot_id,
                        direction: QueueMove::Up,
                    })
                });
            }
            Key::Down if key.modifiers.contains(tuirealm::event::KeyModifiers::SHIFT) => {
                return self.selected_slot().map(|(scope, slot_id)| {
                    Msg::Queue(QueueRequest::Move {
                        scope,
                        slot_id,
                        direction: QueueMove::Down,
                    })
                });
            }
            Key::Char('t')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                return Some(Msg::Shell(ShellRequest::QueueIntent(
                    QueueIntent::StopRemoteTracking,
                )));
            }
            Key::Char('r')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                return Some(Msg::Shell(ShellRequest::QueueIntent(
                    QueueIntent::ReanchorRemoteTracking,
                )));
            }
            Key::Char('z')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                return Some(Msg::Queue(QueueRequest::Undo { scope: self.scope }));
            }
            Key::Char('.') if key.modifiers.is_empty() => {
                // `.` is a selection-dependent chord the focused component
                // owns (CONTEXT.md "Global chord"): emit the queue context-menu
                // request for the currently selected row.
                return match self.delegate_row_local_input(RowLocalInput::Context, None) {
                    RowLocalOutcome::External(RowIntent::Context(slot_id)) => {
                        Some(Msg::Shell(ShellRequest::QueueContextMenu {
                            slot_id: Some(slot_id),
                        }))
                    }
                    _ => Some(Msg::Shell(ShellRequest::QueueContextMenu { slot_id: None })),
                };
            }
            Key::Char('i') => {
                return self.selected_slot().map(|(scope, slot_id)| {
                    Msg::Shell(ShellRequest::QueueIntent(QueueIntent::Navigate {
                        scope,
                        slot_id,
                    }))
                });
            }
            Key::Char('p') => {
                return Some(Msg::Shell(ShellRequest::QueueIntent(QueueIntent::PlayNow)));
            }
            Key::Char('s')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                return Some(Msg::Shell(ShellRequest::QueueIntent(
                    QueueIntent::SavePlaylist,
                )));
            }
            Key::Char('c') if !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) => {
                return Some(Msg::Shell(ShellRequest::QueueIntent(QueueIntent::Clear)));
            }
            _ => {}
        }
        None
    }

    /// Gesture recognition (click / double-click / right-click / wheel) comes
    /// from the private `MouseGestureState` (ADR 0024, design.md D3). Row
    /// identity comes from the embedded control's retained current-frame
    /// point resolution
    /// (design.md D6); scope pills from `scope_regions`. The component emits a
    /// semantic `Msg` with a resolved `QueueSlotId`/scope — never raw
    /// coordinates — except the context-menu anchor (design.md D4).
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // Queue does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => {
                if !self.carrier.claims_current_point(at) {
                    return None;
                }
                self.delegate_row_local_input(RowLocalInput::Wheel { at, delta }, None);
                // Return a framework-visible claim after mutating local state;
                // dropping the message would let the framework's mutation be
                // discarded by the mouse fold.
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MouseGesture::Click(at) => {
                if let Some(scope) = self.claim_scope_pill(at) {
                    return Some(Msg::Shell(ShellRequest::QueueScopeClick { scope }));
                }
                if !self.carrier.claims_current_point(at) {
                    return None;
                }
                let target = self.carrier.resolve_current_point(at).copied();
                if let Some(target) = target {
                    self.delegate_row_local_input(RowLocalInput::Click(at), Some(target));
                }
                self.drag_grab = target;
                Some(Msg::Shell(ShellRequest::QueueRowClick {
                    slot_id: self.carrier.selected_target().copied(),
                }))
            }
            MouseGesture::DoubleClick(at) => {
                if let Some(scope) = self.claim_scope_pill(at) {
                    return Some(Msg::Shell(ShellRequest::QueueScopeClick { scope }));
                }
                if !self.carrier.claims_current_point(at) {
                    return None;
                }
                let target = self.carrier.resolve_current_point(at).copied();
                if let Some(target) = target {
                    self.delegate_row_local_input(RowLocalInput::Click(at), Some(target));
                }
                Some(Msg::Shell(ShellRequest::QueueRowActivate {
                    slot_id: self.carrier.selected_target().copied(),
                }))
            }
            MouseGesture::RightClick(at) => {
                // Legacy parity: a right-click on blank queue space opens no
                // menu. Only resolve a menu when the click lands on a row —
                // never fall back to the prior selection (design.md D4).
                let slot_id = self.carrier.resolve_current_point(at).copied()?;
                self.delegate_row_local_input(RowLocalInput::ContextClick(at), Some(slot_id));
                Some(Msg::Shell(ShellRequest::QueueRowContextMenu {
                    slot_id: Some(slot_id),
                    anchor: (mouse.column, mouse.row),
                }))
            }
            MouseGesture::Drag { to, .. } => {
                let grabbed = self.drag_grab?;
                let resolved = self.carrier.resolve_current_point(to).copied()?;
                if resolved == grabbed {
                    return None;
                }
                self.delegate_row_local_input(RowLocalInput::Click(to), Some(grabbed));
                Some(Msg::Queue(QueueRequest::MoveTo {
                    scope: self.scope,
                    slot_id: grabbed,
                    onto: resolved,
                }))
            }
            MouseGesture::DragEnd => {
                self.drag_grab = None;
                None
            }
        }
    }

    /// If `at` lands on a scope pill, switch the component's own scope and
    /// reset its scroll (design.md D3), and return the new scope.
    fn claim_scope_pill(&mut self, at: Position) -> Option<QueueScope> {
        let &scope = self.scope_regions.resolve(at)?;
        self.scope = scope;
        self.carrier.set_scroll(0);
        Some(scope)
    }

    #[cfg(test)]
    pub(crate) fn test_selected_target(&self) -> Option<QueueSlotId> {
        self.carrier.selected_target().copied()
    }

    #[cfg(test)]
    pub(crate) fn test_cursor(&self) -> usize {
        self.carrier.cursor()
    }

    #[cfg(test)]
    pub(crate) fn test_scroll(&self) -> usize {
        self.carrier.scroll()
    }

    #[cfg(test)]
    pub(crate) fn test_scope_pill_areas(&self) -> (Rect, Rect) {
        (
            self.geometry.scope_local_area,
            self.geometry.scope_remote_area,
        )
    }
}

impl Default for QueueComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QueueComponent {
    fn view(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        // The shell passes the current layout area every frame. Never reuse the
        // previous area when this panel is hidden or resized: stale geometry
        // would repaint the old queue panel and leave a ghost behind.
        self.area = area;
        self.ensure_carrier();
        self.geometry = QueueRenderGeometry::default();
        if let (Some(title_area), Some(title)) = (self.title_area, self.title.as_ref()) {
            render_queue_title_content(frame, title_area, title, &mut self.geometry);
        }
        // Adopt the scope-pill rects the title painter just produced into the
        // irregular-chrome registry (design.md D6).
        self.scope_regions.clear();
        if self.title.is_some() {
            self.scope_regions
                .push(self.geometry.scope_local_area, QueueScope::Local);
            self.scope_regions
                .push(self.geometry.scope_remote_area, QueueScope::Remote);
        }
        render_queue_body(
            frame,
            area,
            QueuePresentation::Wide(self.carrier.wide_mut()),
            self.focused,
            self.throbber,
        );
        if area.height < 1 {
            return;
        }
        if self.carrier.is_empty() {
            frame.render_widget(
                Paragraph::new(self.empty_text.clone())
                    .style(Style::default().fg(palette::TEXT_MUTED)),
                area,
            );
        }
        // The persistent canonical child is the sole Queue body painter and
        // retains the current painted row geometry for later point resolution.
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }
    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            self.focused = matches!(value, AttrValue::Flag(true));
        }
    }
    fn state(&self) -> State {
        State::None
    }
    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for QueueComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        // Keep the shared owner in the presentation the fixed Queue contract
        // selects before any row-local input touches it (design.md D1).
        self.ensure_carrier();
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}

/// Project Queue slots into the canonical provider-neutral row vocabulary
/// (migrate-queue-to-canonical-list D2): a stable `QueueSlotId` target, the
/// slot title, duration/elapsed metadata, and semantic active state whose
/// progress is clamped to `0..=100` at this projection boundary. No ticks,
/// runtime, source, credentials, callbacks, or effects cross the child edge.
pub(in crate::app) fn queue_media_rows(
    slots: &[QueueSlot],
    playback: PlaybackState,
    pending_slot: Option<QueueSlotId>,
) -> Vec<MediaListRow<QueueSlotId>> {
    slots
        .iter()
        .enumerate()
        .map(|(index, slot)| queue_media_row_at(slot, index, playback, pending_slot))
        .collect()
}

pub(in crate::app) fn queue_media_row(
    slot: &QueueSlot,
    index: usize,
    playback: PlaybackState,
    pending_slot: Option<QueueSlotId>,
) -> MediaListRow<QueueSlotId> {
    queue_media_row_at(slot, index, playback, pending_slot)
}

fn queue_media_row_at(
    slot: &QueueSlot,
    index: usize,
    playback: PlaybackState,
    pending_slot: Option<QueueSlotId>,
) -> MediaListRow<QueueSlotId> {
    let is_active = playback.active && playback.active_idx == index;
    let is_pending = pending_slot == Some(slot.slot_id) && !is_active;
    let (title, pos_ticks, duration_ticks) = queue_row_fields(&slot.item, playback, is_active);
    let (semantic_state, trailing) = if is_pending {
        (MediaSemanticState::NowPlaying { progress: None }, None)
    } else if is_active {
        let progress = (pos_ticks > 0 && duration_ticks > 0)
            .then(|| (pos_ticks * 100 / duration_ticks).clamp(0, 100) as u16);
        (
            MediaSemanticState::NowPlaying {
                progress: progress.map(crate::app::components::media_list::ActiveProgress::new),
            },
            None,
        )
    } else {
        let pct = match &slot.item {
            QueueItem::Emby(item) if !item.is_audio() => {
                let pct = fmt_playback_pct(item.playback_position_ticks, item.runtime_ticks);
                (!pct.is_empty()).then_some(pct)
            }
            _ => None,
        };
        (MediaSemanticState::Ordinary, pct)
    };
    MediaListRow::Item {
        target: slot.slot_id,
        primary: title,
        trailing,
        duration: if is_active || is_pending {
            None
        } else {
            let time_text = queue_row_time_text(pos_ticks, duration_ticks, false);
            (!time_text.is_empty()).then_some(time_text)
        },
        kind: MediaKind::Media,
        semantic_state,
    }
}

/// The title and (position, duration) ticks a Queue row paints, resolved per
/// item kind and overridden with live playback ticks for the active row.
fn queue_row_fields(
    item: &QueueItem,
    playback: PlaybackState,
    is_active: bool,
) -> (String, i64, i64) {
    match item {
        QueueItem::Emby(item) => {
            let (pos, runtime) = if is_active {
                (
                    if playback.position_ticks > 0 {
                        playback.position_ticks
                    } else {
                        item.playback_position_ticks
                    },
                    playback.runtime_ticks,
                )
            } else {
                (item.playback_position_ticks, item.runtime_ticks)
            };
            (item.name.clone(), pos, runtime)
        }
        QueueItem::Feed(entry) => (
            entry.title.clone(),
            if is_active {
                playback.position_ticks
            } else {
                0
            },
            entry.duration_ticks.unwrap_or(0) as i64,
        ),
        QueueItem::Audiobookshelf(ep) => (
            ep.title.clone(),
            if is_active {
                playback.position_ticks
            } else {
                0
            },
            ep.duration_ticks.unwrap_or(0) as i64,
        ),
        QueueItem::AudiobookshelfBook(book) => (
            book.title.clone(),
            if is_active {
                playback.position_ticks
            } else {
                0
            },
            book.duration_ticks.unwrap_or(0) as i64,
        ),
    }
}

fn queue_row_time_text(pos_ticks: i64, dur_ticks: i64, show_elapsed: bool) -> String {
    let dur_s = dur_ticks / TICKS_PER_SECOND;
    if dur_s <= 0 {
        return String::new();
    }
    if show_elapsed {
        format!(
            "{} / {}",
            fmt_duration_short(pos_ticks / TICKS_PER_SECOND),
            fmt_duration_short(dur_s)
        )
    } else {
        fmt_duration_short(dur_s)
    }
}
