use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::{
    MediaListCarrier, MediaListRow, MediaListSurfaceInput, MediaListTransition, MediaSemanticState,
};
use super::mouse::gesture::MouseGestureState;
use super::msg::{Msg, QueueRequest};
use super::user_event::UserEvent;
use crate::app::palette;
use crate::app::render::arrangements::queue::{
    queue_footer_row, queue_list_box, queue_panel_subareas,
};
use crate::app::render::components::queue::{render_queue_status, QueueTitleModel};
use crate::app::render::components::widgets::render_queue_panel_frame;
use crate::app::render::{render_queue_body, QueuePresentation};
use crate::app::state::types::playback::{PlaybackState, QueueScope};
use mbv_core::playback_queue::{QueueSlot, QueueSlotId};

mod keys;
mod pointer;
mod rows;

pub(in crate::app) use self::rows::{queue_media_row, queue_media_rows};

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
    /// The panel surface's focused bit, projected per frame from the shell's
    /// panel-focus fact (the same value the legacy base frame used for the
    /// frame fill, task 3.1). The body rows keep the framework focus state;
    /// a mounted overlay blurs the component without changing the column's
    /// focused surface, exactly as before.
    frame_focused: bool,
    empty_text: String,
    area: Rect,
    /// The framed list content area the panel derives from its placement each
    /// `view()` (component-retained geometry, task 3.1): the list body, the
    /// empty-state text, and the context-menu keyboard anchor panel.
    content_area: Rect,
    /// The projected status pill row (playlist source + autosave), pushed by
    /// the queue projection and painted at the panel's own status row.
    status_playlist: Vec<Span<'static>>,
    status_autosave: Option<Vec<Span<'static>>>,
    /// The Local/Remote scope pills (queue concern, painted at the far right
    /// of the same footer row while connected to an mbv-based session).
    status_scope: Option<QueueTitleModel>,
    /// Scope-pill rects retained from the last footer paint.
    scope_local: Option<Rect>,
    scope_remote: Option<Rect>,
    pending_slot: Option<QueueSlotId>,
    drag_grab: Option<QueueSlotId>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle.
    mouse_gestures: MouseGestureState,
}

impl QueueComponent {
    pub(crate) fn clear_selection(&mut self) {
        self.carrier.clear_owner_selection();
    }

    pub(crate) fn selected_row_rect(&self) -> Option<Rect> {
        self.carrier.wide().current_selected_row_rect()
    }

    pub fn new() -> Self {
        Self {
            carrier: MediaListCarrier::new(),
            scope: QueueScope::Local,
            focused: false,
            frame_focused: false,
            empty_text: String::new(),
            area: Rect::default(),
            content_area: Rect::default(),
            status_playlist: Vec::new(),
            status_autosave: None,
            status_scope: None,
            scope_local: None,
            scope_remote: None,
            pending_slot: None,
            drag_grab: None,
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Test-only: drive framework focus the way `Component::attr` does.
    #[cfg(test)]
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn set_content(
        &mut self,
        slots: Vec<QueueSlot>,
        cursor: QueueCursorUpdate,
        scope: QueueScope,
        playback: PlaybackState,
    ) {
        self.set_rows(slots, playback);
        self.set_cursor(cursor);
        self.set_scope(scope);
    }

    /// Replace projected rows while preserving the canonical list's selection.
    pub(in crate::app) fn set_rows(&mut self, slots: Vec<QueueSlot>, playback: PlaybackState) {
        self.carrier
            .set_content(queue_media_rows(&slots, playback, self.pending_slot));
    }

    /// The semantic states of the projected rows, in row order (tick-test
    /// evidence for the shell's now-playing claim projection).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn projected_row_states(&self) -> Vec<MediaSemanticState> {
        self.carrier
            .wide()
            .rows()
            .iter()
            .filter_map(|row| match row {
                MediaListRow::Item { semantic_state, .. } => Some(semantic_state.clone()),
                _ => None,
            })
            .collect()
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

    /// Deliver the current scope independently of row delivery.
    pub(in crate::app) fn set_scope(&mut self, scope: QueueScope) {
        if scope != self.scope {
            self.carrier.set_scroll(0);
        }
        self.scope = scope;
        self.empty_text = if scope == QueueScope::Local {
            "  ¯\\_(ツ)_/¯".into()
        } else {
            "  Remote queue is empty".into()
        };
    }

    pub(in crate::app) fn set_pending_slot(&mut self, slot: Option<QueueSlotId>) {
        self.pending_slot = slot;
    }

    pub(in crate::app) fn set_area(&mut self, area: Rect) {
        self.area = area;
    }

    /// Project the panel surface's focused bit (task 3.1): the same panel-
    /// focus fact the legacy base frame used for the frame fill.
    pub(in crate::app) fn set_frame_focused(&mut self, focused: bool) {
        self.frame_focused = focused;
    }

    /// Project the status pill row (playlist source + autosave) and the
    /// scope pills, painted at the QueueColumn footer below the recessed box.
    pub(in crate::app) fn set_status_pills(
        &mut self,
        playlist: Vec<Span<'static>>,
        autosave: Option<Vec<Span<'static>>>,
        scope: Option<QueueTitleModel>,
    ) {
        self.status_playlist = playlist;
        self.status_autosave = autosave;
        self.status_scope = scope;
    }

    /// The framed list content area the panel retained from its last paint:
    /// the context-menu anchor's panel rect and the list body's own geometry
    /// (task 3.1; the the former queue-area mirror mirror is gone).
    pub(in crate::app) fn content_area(&self) -> Rect {
        self.content_area
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
        input: MediaListSurfaceInput,
        pointer_target: Option<QueueSlotId>,
    ) -> MediaListTransition<QueueSlotId> {
        self.carrier.delegate_operation(
            input
                .into_operation(pointer_target)
                .expect("resolved media-list pointer target"),
        )
    }

    /// The scope/slot pair for Queue's current selection.
    fn selected_slot(&self) -> Option<(QueueScope, QueueSlotId)> {
        self.carrier
            .selected_target()
            .map(|&slot_id| (self.scope, slot_id))
    }

    /// Clamp the shared owner's fixed-row viewport to the queue's content
    /// area. Queue keeps the Wide presentation in every panel mode (spec).
    fn ensure_carrier(&mut self) {
        self.carrier
            .clamp_viewport(self.content_area.height.max(1) as usize);
    }

    #[cfg(test)]
    pub(crate) fn test_scope_pill_areas(&self) -> (Option<Rect>, Option<Rect>) {
        (self.scope_local, self.scope_remote)
    }

    #[cfg(test)]
    pub(crate) fn test_selected_target(&self) -> Option<QueueSlotId> {
        self.carrier.selected_target().copied()
    }

    #[cfg(test)]
    pub(crate) fn test_toggle_selection(&mut self, target: QueueSlotId) {
        self.carrier.toggle_selection(&target);
    }

    #[cfg(test)]
    pub(crate) fn test_multi_selection(&self) -> &[QueueSlotId] {
        self.carrier.multi_selection()
    }

    pub(in crate::app) fn selection_summary(
        &self,
    ) -> crate::app::components::media_list::SelectionSummary {
        self.carrier.selection_summary()
    }

    #[cfg(test)]
    pub(crate) fn test_cursor(&self) -> usize {
        self.carrier.cursor()
    }

    #[cfg(test)]
    pub(crate) fn test_scroll(&self) -> usize {
        self.carrier.scroll()
    }
}

impl Default for QueueComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QueueComponent {
    fn view(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        // The shell passes this panel's `RootFrame`-derived placement every
        // frame (task 3.1). Never reuse the previous placement when this panel
        // is hidden or resized: stale geometry would repaint the old queue
        // panel and leave a ghost behind.
        self.area = area;
        // Component-retained geometry (task 3.1): the framed content area
        // derives from the placement through the shared arrangement
        // helpers, replacing the legacy queue geometry mirror. The list keeps
        // the recessed box's own one-row top inset and bottom padding; the
        // status bar lives in the QueueColumn footer below the box, with one
        // gap row above and below it.
        let footer_row = queue_footer_row(area);
        let panel_box = queue_list_box(area);
        let content_area = queue_panel_subareas(panel_box);
        self.content_area = content_area;
        // The panel fills its own placement: the frame painter covers the
        // whole placement with the QueueColumn surface and the recessed box
        // above the footer band. The footer band keeps the QueueColumn
        // surface around the footer row.
        render_queue_panel_frame(frame, area, self.frame_focused);
        // The status pill row the projection pushed, painted at the
        // QueueColumn footer (moved out of the recessed panel).
        if let Some(footer_row) = footer_row {
            (self.scope_local, self.scope_remote) = render_queue_status(
                frame,
                footer_row,
                self.status_playlist.clone(),
                self.status_autosave.clone(),
                self.status_scope.as_ref(),
            );
        }
        self.ensure_carrier();
        render_queue_body(
            frame,
            content_area,
            QueuePresentation::Wide(self.carrier.wide_mut()),
            self.focused,
        );
        if content_area.height < 1 {
            return;
        }
        if self.carrier.is_empty() && content_area.height > 1 {
            let empty_area = Rect {
                y: content_area.y + 1,
                height: content_area.height - 1,
                ..content_area
            };
            frame.render_widget(
                Paragraph::new(self.empty_text.clone())
                    .style(Style::default().fg(palette::TEXT_EMPHASIS)),
                empty_area,
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
            Event::Keyboard(key) => self.handle_key_result(key).into_option(),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
