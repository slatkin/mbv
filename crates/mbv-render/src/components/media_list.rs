mod row;
mod wide;

/// The shared split-row title policy of `row`, reused by the tree painter.
pub(crate) use row::media_list_title_parts;
/// The flat list's title-parts unit truncation, reused by the tree painter.
pub(crate) use row::truncated_title_spans;

pub use mbv_ui_model::media_list::{
    ActiveProgress, MediaKind, MediaListRow, MediaListTitleReveal, MediaListTrailing,
    MediaSemanticState,
};

use mbv_theme as palette;
use mbv_theme::Surface;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[doc(inline)]
pub use wide::WideMediaListPaintInput;
#[doc(inline)]
pub use wide::render_wide_media_list_component;
/// Which semantic surface should receive the selected-row treatment.
///
/// The policy is deliberately closed: callers choose a named surface identity,
/// never a raw Ratatui style or colour. The painter retains the identity for
/// composition policy while every selected row resolves to the canonical
/// selected-row bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedRowSurface {
    ListBackdrop,
    OwningQueueColumn,
    /// The library Workspace (Wide Hero pane and Library Hero overlay — one
    /// unified look), retained as a closed owning-surface identity.
    OwningLibraryPane,
}

/// Semantic paint policy for one `WideMediaList` view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WideMediaListPaintPolicy {
    focused: bool,
    /// The appearance bit for the zebra fill. Mini view rests the palette
    /// while the list keeps its cursor (`focused`), so the selected row's
    /// bar stays visible on the resting stripe. Defaults to `focused` in
    /// every constructor; only callers that split the bits set it.
    palette_focused: bool,
    selected_surface: SelectedRowSurface,
    zebra: Option<Surface>,
}

impl WideMediaListPaintPolicy {
    #[must_use]
    pub const fn new(focused: bool) -> Self {
        Self {
            focused,
            palette_focused: focused,
            selected_surface: SelectedRowSurface::ListBackdrop,
            zebra: None,
        }
    }

    #[must_use]
    pub const fn for_queue(focused: bool) -> Self {
        Self {
            focused,
            palette_focused: focused,
            selected_surface: SelectedRowSurface::OwningQueueColumn,
            zebra: None,
        }
    }

    #[must_use]
    pub const fn for_library_workspace(focused: bool) -> Self {
        Self {
            focused,
            palette_focused: focused,
            selected_surface: SelectedRowSurface::OwningLibraryPane,
            zebra: None,
        }
    }

    /// Override the zebra palette bit for a list whose cursor stays live
    /// while its palette rests (mini view). Callers that do not split the
    /// bits never need this: the constructors default the palette to the
    /// interaction focus.
    #[must_use]
    pub const fn with_palette_focus(mut self, palette_focused: bool) -> Self {
        self.palette_focused = palette_focused;
        self
    }

    #[must_use]
    pub const fn with_zebra(mut self, zebra: Surface) -> Self {
        self.zebra = Some(zebra);
        self
    }

    #[must_use]
    pub fn zebra_bg(self) -> Option<Color> {
        self.zebra.map(|surface| {
            palette::surface_colors(surface, self.palette_focused)
                .fill
                .to_ratatui()
        })
    }

    #[must_use]
    pub const fn focused(self) -> bool {
        self.focused
    }

    #[must_use]
    pub const fn selected_surface(self) -> SelectedRowSurface {
        self.selected_surface
    }
}
/// Flow-space geometry for a painted media-list control.
///
/// Rows contain the source-row lookup used by painters and an optional stable
/// target for hit maps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowGeometry<Target> {
    offset: usize,
    rows: Vec<Option<Target>>,
    selected_row: Option<usize>,
}

impl<Target> RowGeometry<Target> {
    /// Display-row index at the viewport top.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Number of rows in the complete painted flow.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the painted flow has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Display-row index of the selected row in flow space.
    #[must_use]
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row
    }

    fn row_rect(&self, area: Rect, flow_row: usize) -> Rect {
        Rect {
            y: area.y
                + u16::try_from(flow_row - self.offset)
                    .expect("visible row offset is bounded by the painted rect height"),
            height: 1,
            ..area
        }
    }

    /// The selected row's absolute one-line rectangle when it is visible.
    #[must_use]
    pub fn selected_row_rect(&self, area: Rect) -> Option<Rect> {
        let row = self.selected_row?;
        (self.offset..self.offset.saturating_add(area.height as usize))
            .contains(&row)
            .then(|| self.row_rect(area, row))
    }

    /// Produce target-bearing rectangles for the visible portion of the
    /// completed fixed-row paint. Structural rows remain in the flow but do
    /// not enter retained hit geometry.
    ///
    /// # Panics
    ///
    /// Panics only if a visible row offset exceeds the bounded content rect.
    #[must_use]
    pub fn target_rects(&self, claim_rect: Rect, content_rect: Rect) -> Vec<(Rect, Target)>
    where
        Target: Clone,
    {
        let end = self
            .offset
            .saturating_add(content_rect.height as usize)
            .min(self.rows.len());
        (self.offset..end)
            .filter_map(|flow_row| {
                let target = self.rows.get(flow_row)?.clone()?;
                // Rows anchor at `content_rect.y` (where the list paints)
                // but inherit x/width from the claim rectangle.
                let rect = Rect {
                    y: content_rect.y
                        + u16::try_from(flow_row - self.offset)
                            .expect("visible row offset is bounded by the painted rect height"),
                    height: 1,
                    ..claim_rect
                };
                Some((rect, target))
            })
            .collect()
    }
}

impl<Target: Clone> RowGeometry<Target> {
    pub fn source(
        rows: &[MediaListRow<Target>],
        offset: usize,
        selected_row: Option<usize>,
    ) -> Self {
        Self {
            offset,
            rows: rows
                .iter()
                .map(|row| row.selectable_target().cloned())
                .collect(),
            selected_row,
        }
    }
}
/// The Queue's row palette, shared by the Queue and Grouped Music tree so
/// their base and zebra fills cannot drift apart. The base fill is the
/// recessed `QueuePanel` surface; the stripe is the `QueueColumn` surface.
#[must_use]
pub fn queue_row_background(focused: bool) -> Color {
    palette::surface_colors(palette::Surface::QueuePanel, focused)
        .fill
        .to_ratatui()
}

/// The text a list's marquee clock keys on for one row: the full title text the
/// painter marquees — a split row's context text and item title, one space
/// apart — or `None` for a row with no title. One formula for both sides: the
/// presenter keys its clock with this text and hands the painter the same
/// string, so the two cannot drift. A drift makes the clock restart every frame
/// and the marquee hold at the start forever.
pub fn row_marquee_key<Target>(row: &MediaListRow<Target>) -> Option<String> {
    match row {
        MediaListRow::Item {
            primary, secondary, ..
        } => Some(match secondary.as_deref().filter(|sec| !sec.is_empty()) {
            Some(sec) => format!("{primary} {sec}"),
            None => primary.clone(),
        }),
        MediaListRow::Heading { .. } | MediaListRow::Spacer => None,
    }
}
