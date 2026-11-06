use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::{
    MediaListSurfaceInput, MediaListTransition, WidePresentationState, hit_region,
};
use super::mouse::gesture::MouseGestureState;
use mbv_queue::{QueueSlot, QueueSlotId};
use mbv_render::arrangements::queue::{queue_footer_row, queue_list_content};
#[cfg(any(test, feature = "test"))]
use mbv_render::components::media_list::MediaSemanticState;
use mbv_render::components::media_list::{MediaListRow, WideMediaListPaintPolicy};
use mbv_render::components::queue::{render_empty_queue, render_queue_status};
use mbv_render::components::widgets::render_queue_panel_frame;
use mbv_surfaces::queue::{QueueCursorUpdate, QueueFrameReport, QueueHitTarget, QueueSurface};
use mbv_surfaces::{HitMap, ListCapacity};
use mbv_theme::Surface;
use mbv_ui_model::playback::{PlaybackState, QueueLink};
use mbv_ui_msg::Msg;
use mbv_ui_msg::UserEvent;

mod pointer;

#[doc(inline)]
pub use mbv_ui_model::{queue_media_row, queue_media_rows};

#[derive(Debug)]
pub struct QueueComponent {
    /// The Queue's Surface state (extract-client-core design D5): the canonical
    /// owner over the queue's stable slot identities plus the queue's cursor,
    /// mark, selection, viewport, and key rules. The adapter paints from it
    /// and forwards neutral input; it holds no copy of a state value.
    surface: QueueSurface,
    /// The Wide presentation's TUI-side half: the paint policy, the configured
    /// rectangles, and the retained painted geometry. The canonical state it
    /// paints lives in `surface`.
    wide: WidePresentationState<QueueSlotId>,
    focused: bool,
    /// The panel surface's focused bit, projected per frame from the shell's
    /// panel-focus fact (the same value the legacy base frame used for the
    /// frame fill, task 3.1). The body rows keep the framework focus state;
    /// a mounted overlay blurs the component without changing the column's
    /// focused surface, exactly as before.
    frame_focused: bool,
    /// Mini view suppresses the focused palette unless `frame_focused` says
    /// otherwise. Projected per frame from the shell's breakpoint; only the
    /// palette follows it, never input.
    mini_view: bool,
    empty_text: String,
    area: Rect,
    /// The framed list content area the panel derives from its placement each
    /// `view()` (component-retained geometry, task 3.1): the list body, the
    /// empty-state text, and the context-menu keyboard anchor panel.
    content_area: Rect,
    /// The projected status pill row (owner indicator, playlist source,
    /// autosave prefix, dirty suffix), pushed by the queue projection and
    /// painted at the panel's own status row.
    status_owner: Vec<Span<'static>>,
    status_playlist: Vec<Span<'static>>,
    status_autosave: Option<Vec<Span<'static>>>,
    status_dirty: Option<Vec<Span<'static>>>,
    /// The playlist pill's retained hit area from the last paint.
    playlist_hit: Rect,
    pending_slot: Option<QueueSlotId>,
    drag_grab: Option<QueueSlotId>,
    /// Private per-parent gesture recognition state (ADR 0024, design.md D3).
    /// The gesture policy itself lives in `mbv_surfaces::gesture`.
    mouse_gestures: MouseGestureState,
}

impl QueueComponent {
    pub fn clear_selection(&mut self) {
        self.surface.clear_selection();
    }

    /// Reset the Queue's local presentation (cursor, marks, scroll) without
    /// replacing its rows, link or playback projection. A transient reorder
    /// grab is dropped so no pointer gesture survives the reset.
    pub fn reset_presentation(&mut self) {
        self.surface.reset_presentation();
        self.wide.invalidate_paint();
        self.drag_grab = None;
    }

    #[must_use]
    pub fn selected_row_rect(&self) -> Option<Rect> {
        self.wide.current_selected_row_rect()
    }

    #[must_use]
    pub fn new() -> Self {
        Self {
            surface: QueueSurface::new(),
            wide: WidePresentationState::new(),
            focused: false,
            frame_focused: false,
            mini_view: false,
            empty_text: String::new(),
            area: Rect::default(),
            content_area: Rect::default(),
            status_owner: Vec::new(),
            status_playlist: Vec::new(),
            status_autosave: None,
            status_dirty: None,
            playlist_hit: Rect::default(),
            pending_slot: None,
            drag_grab: None,
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Replace projected rows while preserving the canonical list's selection.
    pub fn set_rows(&mut self, slots: &[QueueSlot], playback: PlaybackState) {
        self.surface
            .list_mut()
            .set_content(queue_media_rows(slots, playback, self.pending_slot));
        self.wide.invalidate_paint();
    }

    /// The semantic states of the projected rows, in row order (tick-test
    /// evidence for the shell's now-playing claim projection).
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn projected_row_states(&self) -> Vec<MediaSemanticState> {
        self.surface
            .rows()
            .iter()
            .filter_map(|row| match row {
                MediaListRow::Item { semantic_state, .. } => Some(semantic_state.clone()),
                _ => None,
            })
            .collect()
    }

    /// Patch one projected row by stable target without rebuilding the list.
    pub fn set_row_patch(&mut self, target: QueueSlotId, row: MediaListRow<QueueSlotId>) -> bool {
        let patched = self.surface.list_mut().patch_row(&target, row);
        self.wide.invalidate_paint();
        patched
    }

    /// Deliver an authoritative cursor command independently of row delivery
    /// through the Queue Surface state's shell-push seam (design.md D5). The
    /// resting scroll-to-cursor clamp runs only inside the `Set` arm;
    /// `Preserve` leaves a freely scrolled viewport where the wheel left it.
    pub fn set_cursor(&mut self, cursor: &QueueCursorUpdate) {
        if self.surface.set_cursor(cursor) {
            self.wide.invalidate_paint();
        }
    }

    /// Deliver the current link independently of row delivery.
    pub fn set_link(&mut self, link: QueueLink) {
        if self.surface.set_link(link) {
            self.wide.invalidate_paint();
        }
        self.empty_text = if link == QueueLink::Local {
            "¯\\_(ツ)_/¯".into()
        } else {
            "Remote queue is empty".into()
        };
    }

    pub fn set_pending_slot(&mut self, slot: Option<QueueSlotId>) {
        self.pending_slot = slot;
    }

    pub fn set_area(&mut self, area: Rect) {
        self.area = area;
    }

    /// Project the panel surface's focused bit (task 3.1): the same panel-
    /// focus fact the legacy base frame used for the frame fill; carries the
    /// shell's appearance-focus decision (`panel_appearance_focus`).
    pub fn set_frame_focused(&mut self, focused: bool) {
        self.frame_focused = focused;
    }

    /// Project the mini-view palette suppression (see the `mini_view` field).
    pub fn set_mini_view(&mut self, mini_view: bool) {
        self.mini_view = mini_view;
    }

    /// Project the status pill row: the owner indicator (always shown, design
    /// D7), the playlist source, the autosave prefix, and the dirty suffix,
    /// painted at the `QueueColumn` footer below the recessed box.
    pub fn set_status_pills(
        &mut self,
        owner: Vec<Span<'static>>,
        playlist: Vec<Span<'static>>,
        autosave: Option<Vec<Span<'static>>>,
        dirty: Option<Vec<Span<'static>>>,
    ) {
        self.status_owner = owner;
        self.status_playlist = playlist;
        self.status_autosave = autosave;
        self.status_dirty = dirty;
    }

    /// The playlist pill's retained hit area from the last paint.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn playlist_hit_for_test(&self) -> Rect {
        self.playlist_hit
    }

    /// The framed list content area the panel retained from its last paint:
    /// the context-menu anchor's panel rect and the list body's own geometry
    /// (task 3.1; the the former queue-area mirror mirror is gone).
    #[must_use]
    pub fn content_area(&self) -> Rect {
        self.content_area
    }

    /// The one seam through which Queue offers an already-normalized row-local
    /// pointer gesture to the Surface state. The state applies the local
    /// transition and returns the closed provider-neutral outcome; the
    /// adapter's key rules live in the Surface state, and pointer targets
    /// resolve here until the hit-map seam lands (row 4.4).
    fn delegate_row_local_input(
        &mut self,
        input: MediaListSurfaceInput,
        pointer_target: Option<QueueSlotId>,
    ) -> MediaListTransition<QueueSlotId> {
        let operation = input
            .into_operation(pointer_target)
            .expect("resolved media-list pointer target");
        let viewport_len = self.wide.painted_viewport_len();
        self.surface
            .delegate_operation_with_viewport_len(operation, viewport_len)
    }

    /// Clamp the Surface state's fixed-row viewport to the queue's content
    /// area. Queue keeps the Wide presentation in every panel mode (spec).
    fn ensure_surface_viewport(&mut self) {
        if self
            .surface
            .set_viewport_height(self.content_area.height.max(1) as usize)
        {
            self.wide.invalidate_paint();
        }
    }

    #[must_use]
    pub fn selection_summary(&self) -> mbv_ui_msg::SelectionSummary {
        self.surface.selection_summary()
    }

    /// The current frame's interaction report (design D6): the capacity the
    /// painted list reports and the hit map over the painted geometry. The
    /// regions are the last completed paint's rows plus the playlist pill,
    /// resolved in the same retained geometry `queue/pointer.rs` resolves
    /// against; the capacity is the viewport height the Surface state clamps
    /// and strides with. The shell and the Client core read this report;
    /// neither recomputes the geometry.
    #[must_use]
    pub fn frame_report(&self) -> QueueFrameReport {
        let mut hit_map = HitMap::new();
        for (region, target) in self.wide.frame_hit_map().iter() {
            hit_map.push(*region, QueueHitTarget::Slot(*target));
        }
        if self.playlist_hit.width > 0 {
            hit_map.push(hit_region(self.playlist_hit), QueueHitTarget::PlaylistPill);
        }
        QueueFrameReport {
            capacity: ListCapacity::new(self.surface.viewport_height()),
            hit_map,
        }
    }

    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn test_cursor(&self) -> usize {
        self.surface.cursor()
    }

    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn test_scroll(&self) -> usize {
        self.surface.scroll()
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
        // helpers, replacing the legacy queue geometry mirror. The list
        // begins one pad row below the box's first row (the box sits below
        // the playback band's gap row) and ends one row above the box's
        // bottom edge, whose spacer row keeps the box's own fill.
        let footer_row = queue_footer_row(area);
        let content_area = queue_list_content(area);
        self.content_area = content_area;
        // The panel fills its own placement: the frame painter covers the
        // whole placement with the QueueColumn surface and the recessed box
        // above the footer band. The footer band keeps the QueueColumn
        // surface around the footer row.
        render_queue_panel_frame(frame, area, self.frame_focused);
        // The status pill row the projection pushed, painted at the
        // QueueColumn footer (moved out of the recessed panel). The painter
        // returns the playlist pill's hit area for click resolution.
        if let Some(footer_row) = footer_row {
            let regions = render_queue_status(
                frame,
                footer_row,
                self.status_owner.clone(),
                self.status_playlist.clone(),
                self.status_autosave.clone(),
                self.status_dirty.clone(),
            );
            self.playlist_hit = regions.playlist;
        } else {
            self.playlist_hit = Rect::default();
        }
        self.ensure_surface_viewport();
        // The cursor follows interaction focus; the palette follows the
        // appearance decision in mini view (`frame_focused`).
        self.wide.set_paint_policy(
            WideMediaListPaintPolicy::for_queue(self.focused)
                .with_palette_focus(
                    self.focused
                        && !crate::mini_palette_suppressed(self.mini_view, self.frame_focused),
                )
                .with_zebra(Surface::QueueColumn),
        );
        super::media_list::render_wide_media_list(
            self.surface.list_mut(),
            &mut self.wide,
            frame,
            content_area,
        );
        if content_area.height < 1 {
            return;
        }
        if self.surface.is_empty() {
            render_empty_queue(frame, content_area, &self.empty_text);
        }
        // The persistent canonical child is the sole Queue body painter and
        // retains the current painted row geometry for later point resolution.
    }

    fn query(&self, _attr: Attribute) -> Option<QueryResult<'_>> {
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
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        // Keep the Surface state in the presentation the fixed Queue contract
        // selects before any row-local input touches it (design.md D1).
        self.ensure_surface_viewport();
        match ev {
            Event::Keyboard(key) => self
                .surface
                .key(crate::root::neutral_key(*key))
                .into_option(),
            Event::Mouse(mouse) => self.handle_mouse(*mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
