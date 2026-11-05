//! The GUI Client's Queue panel (row 6.4): the egui `Painter` over the
//! Queue Surface state, with the fixed-row list, the active-slot mark, the
//! cursor, the marks, the empty-queue state, and the footer's status pills
//! (design D5), every colour resolved through the theme's neutral `Rgb`.
//!
//! The panel holds no interaction state of its own beyond the projection
//! inputs the TUI adapter also holds: cursor, marks, selection, and viewport
//! live in the Surface state, and no stateful egui widget exists here — the
//! interaction state never enters egui's widget memory (design D8). The
//! painter reports each frame's capacity and semantic hit map, and pointer
//! gestures resolve against the last completed frame's report.

use eframe::egui;
use mbv::{FooterPill, GuiSurfaceLeaf, QueueFooterProjection};
use mbv_queue::QueueSlotId;
use mbv_surfaces::gesture::{ClickSelection, PointerGesture};
use mbv_surfaces::media_list::MediaListOperation;
use mbv_surfaces::queue::{QueueFrameReport, QueueHitTarget, QueueSurface};
use mbv_surfaces::{HitMap, HitRegion, ListCapacity};
use mbv_theme::{Role, Surface, surface_colors};
use mbv_ui_model::media_list::{MediaListRow, MediaSemanticState};
use mbv_ui_model::playback::QueueLink;
use mbv_ui_model::queue_rows::queue_media_rows;
use mbv_ui_msg::{ComponentId, KeyEvent, LeafKeyResult, Msg, Position, QueueRequest, ShellRequest};

use mbv::QueueSurfaceProjection;

/// The fixed row height every queue row paints at, in logical points
/// (`specs/gui-client`: fixed row heights so capacity is a whole number of
/// rows).
pub(crate) const ROW_HEIGHT: f32 = 24.0;

/// The padding between the panel edge and the list content, in logical
/// points.
pub(crate) const PANEL_PADDING: f32 = 8.0;

/// The gap between two footer pills, and the inset a pill group paints around
/// its text, in logical points (the TUI's one-cell pad).
pub(crate) const PILL_PAD: f32 = 6.0;

/// The gap between a title's column and the duration column, in logical
/// points.
const COLUMN_GAP: f32 = 8.0;

/// The Queue panel: the Surface state plus the projection inputs the TUI
/// adapter also holds, and the last completed frame's interaction report.
pub(crate) struct QueuePanel {
    surface: QueueSurface,
    /// The adopted pending transition awaiting playback-owner observation; a
    /// row-projection input, not interaction state.
    pending_slot: Option<QueueSlotId>,
    empty_text: String,
    /// The last completed frame's report: the capacity the page stride
    /// derives from and the regions pointer gestures resolve against
    /// (`specs/client-interaction-core`).
    last_report: QueueFrameReport,
    /// The painted list content rect: the wheel's claim area, which spans the
    /// list body rather than single rows (the TUI's wheel claims the panel's
    /// painted geometry the same way).
    list_rect: HitRegion,
    /// The slot a plain press grabbed for a reorder drag (ADR 0024's drag
    /// anchor lives in the Client core's gesture policy; the grabbed target
    /// is this surface's own).
    drag_grab: Option<QueueSlotId>,
    /// The Queue footer's status pills the last sync adopted. `None` before the
    /// first sync, when the footer paints nothing.
    footer: Option<QueueFooterProjection>,
}

impl Default for QueuePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl QueuePanel {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            surface: QueueSurface::new(),
            pending_slot: None,
            empty_text: "¯\\_(ツ)_/¯".into(),
            last_report: QueueFrameReport {
                capacity: ListCapacity::new(0),
                hit_map: HitMap::new(),
            },
            list_rect: HitRegion::new(0.0, 0.0, 0.0, 0.0),
            drag_grab: None,
            footer: None,
        }
    }

    /// Adopt one projection push from the `App` (design D5): the row work the
    /// fingerprint selected, the authoritative cursor push, and the shown
    /// link, in the TUI adapter's delivery order.
    pub(crate) fn sync(&mut self, projection: QueueSurfaceProjection) {
        self.pending_slot = projection.pending_slot;
        if let Some(slots) = projection.slots {
            self.surface.list_mut().set_content(queue_media_rows(
                &slots,
                projection.playback,
                projection.pending_slot,
            ));
        } else if let Some((target, row)) = projection.patch {
            self.surface.list_mut().patch_row(&target, row);
        }
        self.surface.set_cursor(&projection.cursor);
        if self.surface.set_link(projection.link) {
            self.empty_text = if projection.link == QueueLink::Local {
                "¯\\_(ツ)_/¯".into()
            } else {
                "Remote queue is empty".into()
            };
        }
    }

    /// Adopt the Queue footer's projection the `App` computed for this tick
    /// (design D5).
    pub(crate) fn sync_footer(&mut self, footer: QueueFooterProjection) {
        self.footer = Some(footer);
    }

    /// The last completed frame's interaction report.
    #[cfg(test)]
    pub(crate) fn last_report(&self) -> &QueueFrameReport {
        &self.last_report
    }

    /// The cursor as an index into the selectable rows (test evidence).
    #[cfg(test)]
    pub(crate) fn test_cursor(&self) -> usize {
        self.surface.cursor()
    }

    /// The marked-target count (test evidence).
    #[cfg(test)]
    pub(crate) fn test_mark_count(&self) -> usize {
        self.surface.multi_selection().len()
    }

    /// The footer projection the last sync adopted (test evidence).
    #[cfg(test)]
    pub(crate) fn test_footer(&self) -> Option<&QueueFooterProjection> {
        self.footer.as_ref()
    }

    /// Resolve one recognized pointer gesture against the last completed
    /// frame's painted geometry, apply the row-local operation to the Surface
    /// state, and return the message whose App-side effect the Client core
    /// dispatches. A gesture whose position lands outside the painted list
    /// claims nothing.
    pub(crate) fn pointer(&mut self, gesture: PointerGesture<Position>) -> Option<Msg> {
        match gesture {
            PointerGesture::Scroll { at, delta } => {
                if !self.list_rect.contains(at) {
                    return None;
                }
                self.surface.delegate_operation_with_viewport_len(
                    MediaListOperation::Scroll(delta),
                    self.last_report.capacity.whole_items(),
                );
                None
            }
            PointerGesture::Click { at, selection } => self.handle_click(at, selection),
            PointerGesture::DoubleClick(at) => self.handle_double_click(at),
            PointerGesture::Drag { to, .. } => self.handle_drag(to),
            PointerGesture::DragEnd => {
                self.drag_grab = None;
                None
            }
            // The context menu is a surface the first slice does not present
            // (`specs/gui-client`, "A surface is not yet ported"); the
            // gesture stays unclaimed.
            PointerGesture::RightClick(_) => None,
        }
    }

    fn row_target(&self, at: Position) -> Option<QueueSlotId> {
        match self.last_report.hit_map.resolve(at)? {
            QueueHitTarget::Slot(slot_id) => Some(*slot_id),
            // The playlist pill's press opens the Playlists surface, which the
            // GUI does not present; the list target claims nothing for it.
            QueueHitTarget::PlaylistPill => None,
        }
    }

    fn handle_click(&mut self, at: Position, selection: ClickSelection) -> Option<Msg> {
        let target = self.row_target(at)?;
        let operation = match selection {
            ClickSelection::Plain => MediaListOperation::Select(target),
            ClickSelection::Toggle => MediaListOperation::Toggle(target),
            ClickSelection::Range => MediaListOperation::Range(target),
        };
        self.surface.delegate_operation_with_viewport_len(
            operation,
            self.last_report.capacity.whole_items(),
        );
        self.drag_grab = (selection == ClickSelection::Plain).then_some(target);
        Some(Msg::Shell(Box::new(ShellRequest::QueueRowClick {
            slot_id: self.surface.selected_target().copied(),
        })))
    }

    fn handle_double_click(&mut self, at: Position) -> Option<Msg> {
        let target = self.row_target(at)?;
        self.surface.delegate_operation_with_viewport_len(
            MediaListOperation::Activate(target),
            self.last_report.capacity.whole_items(),
        );
        Some(Msg::Shell(Box::new(ShellRequest::QueueRowActivate {
            slot_id: self.surface.selected_target().copied(),
        })))
    }

    fn handle_drag(&mut self, to: Position) -> Option<Msg> {
        let grabbed = self.drag_grab?;
        let resolved = self.row_target(to)?;
        if resolved == grabbed {
            return None;
        }
        self.surface.delegate_operation_with_viewport_len(
            MediaListOperation::Select(grabbed),
            self.last_report.capacity.whole_items(),
        );
        Some(Msg::Queue(QueueRequest::MoveTo {
            link: self.surface.link(),
            slot_id: grabbed,
            onto: resolved,
        }))
    }

    /// Paint one frame over `rect` and report the frame's capacity and hit
    /// map. The footer takes the panel's bottom row below the list. The
    /// viewport height the Surface state clamps with is the painted capacity,
    /// so the page stride the keys derive is the painted one.
    pub(crate) fn paint(&mut self, painter: &egui::Painter, rect: egui::Rect, focused: bool) {
        let (list_rect, footer_rect) = split_bottom(rect.shrink(PANEL_PADDING), ROW_HEIGHT);
        self.list_rect = to_region(list_rect);
        let capacity = ListCapacity::new(rows_fitting(list_rect.height(), ROW_HEIGHT));
        self.surface.set_viewport_height(capacity.whole_items());
        let viewport = self.surface.list().resolve_viewport(capacity.whole_items());

        // The panel paints its whole placement with the Queue column's fill,
        // and the list body keeps the Queue panel surface inside it, exactly
        // as the TUI's recessed box does.
        painter.rect_filled(rect, 0.0, surface_fill(Surface::QueueColumn, focused));
        let marks: Vec<QueueSlotId> = self.surface.multi_selection().to_vec();
        let selected_display_row = self.surface.list().selected_display_row();
        let rows = &self.surface.rows()[viewport.offset..][..(viewport
            .height
            .min(self.surface.rows().len().saturating_sub(viewport.offset)))];
        let mut hit_map = HitMap::new();
        let mut striped = viewport.offset % 2 == 1;
        let mut row_top = list_rect.top();
        for (visible, row) in rows.iter().enumerate() {
            let display_row = viewport.offset + visible;
            let row_rect = egui::Rect::from_min_size(
                egui::pos2(list_rect.left(), row_top),
                egui::vec2(list_rect.width(), ROW_HEIGHT),
            );
            row_top += ROW_HEIGHT;
            let target = row.selectable_target();
            let marked = target.is_some_and(|slot| marks.contains(slot));
            let cursor = selected_display_row == Some(display_row);
            striped =
                paint_row_background(painter, row_rect, row, cursor || marked, striped, focused);
            if let Some(slot) = target {
                hit_map.push(to_region(row_rect), QueueHitTarget::Slot(*slot));
            }
            paint_row_text(painter, row_rect, row, cursor || marked);
        }
        if self.surface.is_empty() {
            paint_empty_queue(painter, list_rect, &self.empty_text);
        }
        if let Some(footer) = &self.footer {
            paint_footer(painter, footer_rect, footer);
        }
        self.last_report = QueueFrameReport { capacity, hit_map };
    }
}

/// One row's background: the cursor's and a marked row's selected-row bar,
/// or the ungrouped list's zebra alternation otherwise — opening on the
/// interior fill, the stripe the Queue column's own fill. Returns the next
/// stripe state.
fn paint_row_background(
    painter: &egui::Painter,
    row_rect: egui::Rect,
    row: &MediaListRow<QueueSlotId>,
    selected: bool,
    striped: bool,
    focused: bool,
) -> bool {
    if selected {
        // The selected row bar is one opaque fill regardless of the owning
        // surface (the TUI's `selected_row_surface_color`).
        painter.rect_filled(row_rect, 0.0, surface_fill(Surface::SelectedRow, false));
        return striped;
    }
    match row {
        MediaListRow::Item { .. } => {
            if striped {
                painter.rect_filled(row_rect, 0.0, surface_fill(Surface::QueueColumn, focused));
            }
            !striped
        }
        MediaListRow::Heading { .. } | MediaListRow::Spacer => striped,
    }
}

/// One row's text: the now-playing marker and title, the inline progress
/// percent, and the duration, in the semantic roles the TUI row painter
/// resolves for the same states.
fn paint_row_text(
    painter: &egui::Painter,
    row_rect: egui::Rect,
    row: &MediaListRow<QueueSlotId>,
    selected: bool,
) {
    let MediaListRow::Item {
        primary,
        duration,
        semantic_state,
        ..
    } = row
    else {
        // The first slice's Queue Surface state projects only item rows.
        return;
    };
    let (now_playing, progress) = match semantic_state {
        MediaSemanticState::NowPlaying { progress } => (true, progress_percent(*progress)),
        MediaSemanticState::Active { progress } => (false, progress_percent(*progress)),
        // A played row and an ordinary row both paint no inline progress.
        MediaSemanticState::Played | MediaSemanticState::Ordinary => (false, None),
    };
    let title_colour = if selected {
        role_colour(Role::SelectedRowFg)
    } else {
        match semantic_state {
            MediaSemanticState::Played => role_colour(Role::TextMuted),
            _ => role_colour(Role::TextEmphasis),
        }
    };
    let mut x = row_rect.left() + PANEL_PADDING;
    let baseline = egui::Align2::LEFT_CENTER;
    let y = row_rect.center().y;
    if now_playing {
        let icon = "▶ ";
        painter.text(
            egui::pos2(x, y),
            baseline,
            icon,
            egui::FontId::proportional(13.0),
            role_colour(Role::Accent),
        );
        x += painter
            .layout_no_wrap(
                icon.to_owned(),
                egui::FontId::proportional(13.0),
                title_colour,
            )
            .size()
            .x;
    }
    let mut title = primary.clone();
    if let Some(percent) = progress {
        title.push(' ');
        title.push_str(&percent);
    }
    // The title's column runs from the marker to one gap short of the
    // duration. A title wider than that column elides with an ellipsis, and
    // the clip rect keeps every glyph inside the column.
    let duration = duration.as_deref().filter(|text| !text.is_empty());
    let duration_width = duration.map_or(0.0, |text| {
        let width = painter
            .layout_no_wrap(
                text.to_owned(),
                egui::FontId::proportional(13.0),
                role_colour(Role::Duration),
            )
            .size()
            .x;
        width + COLUMN_GAP
    });
    let title_right = (row_rect.right() - PANEL_PADDING - duration_width).max(x);
    let mut job = egui::text::LayoutJob::simple_singleline(
        title,
        egui::FontId::proportional(13.0),
        title_colour,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(title_right - x);
    let galley = painter.layout_job(job);
    let column = egui::Rect::from_min_max(
        egui::pos2(x, row_rect.top()),
        egui::pos2(title_right, row_rect.bottom()),
    );
    let origin = baseline.anchor_size(egui::pos2(x, y), galley.size()).min;
    painter
        .with_clip_rect(column)
        .galley(origin, galley, title_colour);
    if let Some(duration) = duration {
        painter.text(
            egui::pos2(row_rect.right() - PANEL_PADDING, y),
            egui::Align2::RIGHT_CENTER,
            duration,
            egui::FontId::proportional(13.0),
            role_colour(Role::Duration),
        );
    }
}

/// The progress percent the TUI row painter appends, as its own text.
fn progress_percent(progress: Option<mbv_ui_model::media_list::ActiveProgress>) -> Option<String> {
    progress.map(|value| format!("{}%", value.percent()))
}

/// The empty-queue state: the emblem-less text the TUI paints centered in
/// bold, in the empty-queue role. The TUI's block-art emblem is a monospace
/// figure; the GUI's proportional family does not align it, so the text
/// alone carries the state.
fn paint_empty_queue(painter: &egui::Painter, list_rect: egui::Rect, text: &str) {
    painter.text(
        list_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(14.0),
        role_colour(Role::EmptyQueueFg),
    );
}

/// One run of a footer pill group: a pill's text, or the pad between two
/// pills, which paints in the pill surface with no text.
enum FooterRun<'a> {
    Gap,
    Pill(&'a FooterPill),
}

/// The Queue footer's status pills (row 6.4's footer, design D5), on the footer
/// band. The left group reads [pad, autosave, pad, playlist, pad]; the right
/// group reads [pad, dirty, pad, owner] and is right-anchored. The right group
/// drops when it would reach the left group, as the TUI footer does. The pills
/// claim no press: the owner is informational, and the playlist pill's press
/// opens the Playlists surface, which the GUI does not present.
fn paint_footer(painter: &egui::Painter, rect: egui::Rect, footer: &QueueFooterProjection) {
    painter.rect_filled(rect, 0.0, surface_fill(Surface::QueuePanelBand, false));
    let left = left_runs(footer);
    let left_width = runs_width(painter, &left);
    paint_runs(painter, rect, rect.left(), &left, left_width);
    let right = right_runs(footer);
    let right_width = runs_width(painter, &right);
    let right_x = rect.right() - right_width;
    if right_x >= rect.left() + left_width {
        paint_runs(painter, rect, right_x, &right, right_width);
    }
}

/// The left pill group's runs: the autosave indicator when present, then the
/// playlist pill.
fn left_runs(footer: &QueueFooterProjection) -> Vec<FooterRun<'_>> {
    let mut runs = vec![FooterRun::Gap];
    if let Some(autosave) = &footer.autosave {
        runs.extend([FooterRun::Pill(autosave), FooterRun::Gap]);
    }
    runs.extend([FooterRun::Pill(&footer.playlist), FooterRun::Gap]);
    runs
}

/// The right pill group's runs: the dirty marker when present, then the owner.
fn right_runs(footer: &QueueFooterProjection) -> Vec<FooterRun<'_>> {
    let mut runs = Vec::new();
    if let Some(dirty) = &footer.dirty {
        runs.extend([FooterRun::Gap, FooterRun::Pill(dirty), FooterRun::Gap]);
    }
    runs.push(FooterRun::Pill(&footer.owner));
    runs
}

/// The painted width of one pill group: its pads and its texts.
fn runs_width(painter: &egui::Painter, runs: &[FooterRun<'_>]) -> f32 {
    runs.iter()
        .map(|run| match run {
            FooterRun::Gap => PILL_PAD,
            FooterRun::Pill(pill) => pill_text_width(painter, pill),
        })
        .sum()
}

/// One pill group at `x` on the footer band: the group's pill surface, then
/// each run's text in its role. The pads advance the cursor without text.
fn paint_runs(
    painter: &egui::Painter,
    band: egui::Rect,
    x: f32,
    runs: &[FooterRun<'_>],
    width: f32,
) {
    painter.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x, band.top()), egui::vec2(width, band.height())),
        0.0,
        surface_fill(Surface::StatusBarPill, false),
    );
    let mut cursor = x;
    for run in runs {
        match run {
            FooterRun::Gap => cursor += PILL_PAD,
            FooterRun::Pill(pill) => {
                painter.text(
                    egui::pos2(cursor, band.center().y),
                    egui::Align2::LEFT_CENTER,
                    &pill.text,
                    pill_font(),
                    role_colour(pill.role),
                );
                cursor += pill_text_width(painter, pill);
            }
        }
    }
}

/// The painted width of one pill's text.
fn pill_text_width(painter: &egui::Painter, pill: &FooterPill) -> f32 {
    painter
        .layout_no_wrap(pill.text.clone(), pill_font(), role_colour(pill.role))
        .size()
        .x
}

/// The font the footer's pills paint in. The bold flag is not painted: the
/// bundled families expose no bold face to select.
fn pill_font() -> egui::FontId {
    egui::FontId::proportional(13.0)
}

/// The whole rows of `row_height` that fit inside `height`, by comparison:
/// the largest count whose rows the height still covers. No float-to-integer
/// cast: the count is small, and the additions stay exact while the partial
/// sums remain representable.
fn rows_fitting(height: f32, row_height: f32) -> usize {
    let mut rows = 0;
    let mut used = row_height;
    while used <= height {
        rows += 1;
        used += row_height;
    }
    rows
}

/// A surface's fill, through the neutral `Rgb` (task 1.3).
pub(crate) fn surface_fill(surface: Surface, focused: bool) -> egui::Color32 {
    to_egui(surface_colors(surface, focused).fill)
}

/// A role's colour, through the neutral `Rgb` (task 1.3).
pub(crate) fn role_colour(role: Role) -> egui::Color32 {
    to_egui(role.color())
}

fn to_egui(rgb: mbv_theme::Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

pub(crate) fn to_region(rect: egui::Rect) -> HitRegion {
    HitRegion::new(rect.left(), rect.top(), rect.width(), rect.height())
}

/// Split `rect` into the part above its bottom `height` points and that bottom
/// part: the Queue list above its footer row, and the window body above the
/// Status bar. The bottom part never exceeds `rect`.
pub(crate) fn split_bottom(rect: egui::Rect, height: f32) -> (egui::Rect, egui::Rect) {
    let split = (rect.bottom() - height).max(rect.top());
    (
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), split)),
        egui::Rect::from_min_max(egui::pos2(rect.left(), split), rect.max),
    )
}

/// The Queue panel as the shared router's leaf: the focused surface the
/// router's `FallThrough` and deferred candidates deliver to, and the
/// surface the recognized pointer gestures resolve against.
impl GuiSurfaceLeaf for QueuePanel {
    fn id(&self) -> ComponentId {
        ComponentId::Queue
    }

    fn visual_selection_active(&self) -> bool {
        self.surface.list().is_visual_mode()
    }

    fn key(&mut self, key: KeyEvent) -> LeafKeyResult {
        self.surface.key(key)
    }

    fn pointer(&mut self, gesture: PointerGesture<Position>) -> Option<Msg> {
        QueuePanel::pointer(self, gesture)
    }
}

#[cfg(test)]
mod tests;
