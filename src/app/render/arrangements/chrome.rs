use super::queue::{queue_panel_geometry, QueuePanelInputs};
use crate::app::layout::FrameChromeGeometry;
use crate::app::render::components::widgets::{queue_panel_inset, COLUMN_GAP};
use crate::app::{PanelFocus, PanelMode, TABBAR_LEFT_RESERVE};
use ratatui::layout::Rect;

/// Height of the tab-bar box: 1 row padding + 1 row tab + 1 row spacer.
const TAB_BAR_BOX_HEIGHT: u16 = 3;

/// Height of the player panel box below the tab bar (seekbar + title +
/// blank trailing row).
pub(in crate::app) const PLAYER_BOX_HEIGHT: u16 = 3;

/// Rows the Queue playback panel always spends on its header in every
/// queue-visible layout, idle included (design D10; the painted row lands
/// with task 3.2). The root placement reserves it now so the Queue panel's
/// placement already starts below the Queue playback panel's header. Two
/// rows: the header text row plus the `queue_panel_inset` row of column
/// padding above it, which recesses the header from the column's top edge.
pub(in crate::app) const QUEUE_PLAYBACK_HEADER_ROWS: u16 = 2;

/// Rows the right column reserves at the bottom for the floating status
/// bar: one gap row, the status row, one padding row below it, the same
/// floating shape as the QueueColumn footer.
pub(in crate::app) const STATUS_BAR_BAND_HEIGHT: u16 = 3;

/// The status row inside its reserved band: two columns of padding each
/// side plus one gap row above it, so the bar floats clear of the content
/// above and the library column's edges instead of touching them (the
/// QueueColumn footer's inset).
pub(in crate::app) fn status_bar_row(band: Rect) -> Rect {
    Rect {
        x: band.x + 2,
        y: band.y.saturating_add(1),
        width: band.width.saturating_sub(4),
        height: 1,
    }
}

/// Columns at which the queue column's visual slot and transport render
/// side by side rather than stacked (the folded change's placement rule).
const QUEUE_PLAYBACK_WIDE_COLUMNS: u16 = 100;

/// The gap between the side-by-side visual slot and the transport.
const SLOT_TRANSPORT_GAP: u16 = 2;

/// Resolved app state needed to place the frame chrome for one terminal area.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct ChromeGeometryInput {
    pub area: Rect,
    pub panel_mode: PanelMode,
    pub panel_focus: PanelFocus,
    pub queue_column_width: u16,
    pub terminal_width: u16,
    /// The queue visual slot's size as published by the last full frame
    /// (`AppLayout::main.card`). The slot's authoritative size is paint-time
    /// (the image protocol resolves it); the root placements and the queue
    /// geometry share the row count through `queue_playback_rows`, while the
    /// draw path's `Queue playback` paint recomputes it from the frame's
    /// freshly published slot size.
    pub card_height: u16,
    /// Whether playback is active. Idle collapses the visual slot and the
    /// transport to zero rows (task 3.6): the connected-idle exception is
    /// deleted, so a connected but idle transport keeps only the header row.
    pub playback_active: bool,
    /// Whether the queue column's transport projects a two-part now-playing
    /// title (a context part): the title band expands onto its third row,
    /// so the transport's footprint grows one row (the expanded title band).
    pub queue_title_expanded: bool,
}

/// Inner tab-strip text width for a tab-bar box of `tab_bar_width` columns.
///
/// Shared by the tab bar painter (`render_tab_bar`, task 2.1) and the
/// keyboard-path `App::ensure_tab_visible`, so the tab-scroll math matches
/// what is painted rather than being re-derived (ADR 0022 Residual A).
/// `PB_H` is the 2-column padding inside the coloured box, applied on both
/// sides.
pub(in crate::app) fn tab_strip_text_width(tab_bar_width: u16) -> u16 {
    const PB_H: u16 = 2;
    tab_bar_width.saturating_sub(2 * PB_H + TABBAR_LEFT_RESERVE)
}

/// The root frame's panel placements for one Panel mode (design D1, task 1.3):
/// **data only** — the draw path still composes the legacy base frame; later
/// slices consume these placements panel by panel. A panel absent from the
/// current mode has no placement (`None`), never an empty rect. Mount rule
/// (design D1): Tab, Library and Status bar placements exist only when the
/// library column is visible; Library playback only when the queue column is
/// hidden; Queue and Queue playback only when the queue column is visible;
/// Queue boundary only in the two-panel layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PanelPlacement {
    Tab(Rect),
    Library(Rect),
    LibraryPlayback(Rect),
    Queue(Rect),
    QueuePlayback(Rect),
    StatusBar(Rect),
    QueueBoundary(Rect),
}

impl PanelPlacement {
    #[cfg(test)]
    pub(crate) fn rect(self) -> Rect {
        match self {
            Self::Tab(rect)
            | Self::Library(rect)
            | Self::LibraryPlayback(rect)
            | Self::Queue(rect)
            | Self::QueuePlayback(rect)
            | Self::StatusBar(rect)
            | Self::QueueBoundary(rect) => rect,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RootFrame {
    /// Tab bar at the top of the library column (`TabPanel`, task 2.1).
    pub tab: Option<Rect>,
    /// The library column's content area (below the tab bar; below the
    /// playback strip in LibraryOnly) (`LibraryPanel`, task 4.1). The
    /// strip's `PLAYER_BOX_HEIGHT` band is reserved only where the strip
    /// paints (task 4.1): in LibraryOnly the library starts below it; in a
    /// queue-visible layout the library starts at the tab bar's bottom edge
    /// and no strip rows are reserved (the frame's one transport is the
    /// Queue playback panel's).
    pub library: Option<Rect>,
    /// The right-column playback strip (`LibraryPlaybackPanel`, task 4.1):
    /// placed — and its `PLAYER_BOX_HEIGHT` rows reserved — only when the
    /// queue column is hidden.
    pub library_playback: Option<Rect>,
    /// The queue panel's placement below the queue column's playback region
    /// (`QueuePanel`, task 3.1).
    pub queue: Option<Rect>,
    /// The queue column's playback region above the queue panel: the header
    /// row plus the visual slot/transport rows (`QueuePlaybackPanel`, task
    /// 3.5). Sized paint-free from the last published card height until the
    /// visual slot moves into the panel (tasks 3.4/3.5).
    pub queue_playback: Option<Rect>,
    /// The one-row status bar at the bottom of the library column
    /// (`StatusBarPanel`, task 2.2).
    pub status_bar: Option<Rect>,
    /// The one-column queue boundary between the two panels (the mounted
    /// `QueueBoundaryComponent`), placed only in the two-panel layout.
    pub queue_boundary: Option<Rect>,
}

impl RootFrame {
    /// Returns placements in paint order. Named fields remain convenient
    /// accessors for sync, hit testing, and tests.
    pub(crate) fn placements(self) -> [Option<PanelPlacement>; 7] {
        [
            self.tab.map(PanelPlacement::Tab),
            self.library_playback.map(PanelPlacement::LibraryPlayback),
            self.library.map(PanelPlacement::Library),
            self.queue_playback.map(PanelPlacement::QueuePlayback),
            self.queue.map(PanelPlacement::Queue),
            // Status bar is last among content panels so it cannot be covered
            // by the library surface above it.
            self.status_bar.map(PanelPlacement::StatusBar),
            self.queue_boundary.map(PanelPlacement::QueueBoundary),
        ]
    }
}

/// A panel placement that exists only when its mount-rule condition holds and
/// the computed rect is paintable: a panel absent from a mode has no
/// placement, never an empty rect (design D1's live-ICF mount rule).
fn placed_when(cond: bool, rect: Rect) -> Option<Rect> {
    cond.then_some(rect).filter(|r| r.width > 0 && r.height > 0)
}

/// Whether the queue column is wide enough for the side-by-side visual slot
/// and transport arrangement. One source for the breakpoint: the root
/// placements and the Queue playback panel's paint step both call this.
pub(in crate::app) fn queue_playback_column_wide(left_area_width: u16) -> bool {
    left_area_width >= QUEUE_PLAYBACK_WIDE_COLUMNS
}

/// The transport rect for one painted visual slot, within the queue playback
/// rows below the header row (`slot_region`'s top row): side by side at 100+
/// columns with the 2-cell gap, stacked below the slot otherwise. One
/// function the root placements (through `queue_playback_rows`) and the
/// Queue playback panel's paint step both call, so the breakpoint, the gap,
/// and the side-by-side/stacked split cannot drift between them (review of
/// tasks 3.5-3.8).
pub(in crate::app) fn queue_playback_transport_area(
    slot_region: Rect,
    column_wide: bool,
    card_width: u16,
    card_height: u16,
    title_expanded: bool,
) -> Rect {
    // The expanded title band (a context part plays) spends one more row:
    // controls + pills, show + pos/dur, title.
    let player_rows = PLAYER_BOX_HEIGHT + u16::from(title_expanded);
    if column_wide {
        Rect {
            x: slot_region
                .x
                .saturating_add(card_width)
                .saturating_add(SLOT_TRANSPORT_GAP),
            y: slot_region.y,
            width: slot_region
                .width
                .saturating_sub(card_width + SLOT_TRANSPORT_GAP),
            height: card_height.max(player_rows),
        }
    } else {
        Rect {
            x: slot_region.x,
            y: slot_region.y.saturating_add(card_height),
            width: slot_region.width,
            height: player_rows,
        }
    }
}

/// Rows the queue column spends on the visual slot and the transport, above
/// the Queue panel (the header row is a separate, always-spent input). Stacked
/// below 100 columns: slot rows plus the transport's `PLAYER_BOX_HEIGHT`;
/// 100+ side by side: the taller of the two governs. Idle collapses both to
/// zero rows (task 3.6); paused counts as active. Shared by the root
/// placements and the Queue playback panel's draw-path paint.
pub(in crate::app) fn queue_playback_rows(
    column_wide: bool,
    card_height: u16,
    playback_active: bool,
    title_expanded: bool,
) -> u16 {
    if !playback_active {
        return 0;
    }
    // The rows are the transport's footprint in the slot region (the rows
    // below the header row): side by side it starts on the slot region's
    // first row, so its height governs; stacked it starts after the slot's
    // rows, so its offset plus height governs. Derived from
    // `queue_playback_transport_area` over an unbounded probe region (with
    // the slot collapsed to zero width) so the row count and the painted
    // transport rect cannot drift (review of tasks 3.5-3.8).
    queue_playback_transport_area(
        Rect::new(0, 0, u16::MAX, u16::MAX),
        column_wide,
        0,
        card_height,
        title_expanded,
    )
    .bottom()
}

/// Computes the root/chrome geometry for one frame without reading app state.
pub(in crate::app) fn chrome_geometry(input: ChromeGeometryInput) -> FrameChromeGeometry {
    let area = input.area;
    // Left panel (card + queue) | Right panel (library, remaining).
    let left_w = match input.panel_mode {
        PanelMode::Both => input.queue_column_width,
        PanelMode::LibraryOnly => 0,
        PanelMode::QueueOnly => area.width,
    };
    let right_w = area.width.saturating_sub(left_w);
    let right_visible = input.panel_mode != PanelMode::QueueOnly;

    let content_h = area.height;
    let left_area = if input.panel_mode == PanelMode::LibraryOnly {
        Rect::default()
    } else {
        Rect {
            x: area.x,
            y: area.y,
            width: left_w,
            height: content_h,
        }
    };
    #[cfg(test)]
    let queue_focused = matches!(input.panel_focus, PanelFocus::Queue);
    let panel_area = if input.terminal_width < crate::app::MINI_VIEW_THRESHOLD
        && input.panel_mode == PanelMode::LibraryOnly
    {
        area
    } else {
        left_area
    };
    // Inner content area with padding inside the colored box (queue uses this).
    let left_content = queue_panel_inset(left_area);

    let tab_h: u16 = TAB_BAR_BOX_HEIGHT;
    // The playback strip's rows are reserved only where the strip paints
    // (task 4.1, D10): a `PLAYER_BOX_HEIGHT` band between the tab bar and
    // the library in LibraryOnly, nothing in a queue-visible layout — there
    // the frame's one transport is the Queue playback panel's, and the
    // library reclaims the band.
    let strip_h = if input.panel_mode == PanelMode::LibraryOnly {
        PLAYER_BOX_HEIGHT
    } else {
        0
    };
    let right_area = Rect {
        x: area.x + left_w + COLUMN_GAP,
        y: area.y + tab_h + strip_h,
        width: right_w.saturating_sub(COLUMN_GAP),
        height: content_h
            .saturating_sub(STATUS_BAR_BAND_HEIGHT)
            .saturating_sub(tab_h)
            .saturating_sub(strip_h),
    };

    // The playback strip's band below the tab bar (library column only;
    // task 4.1).
    let player_area = if input.panel_mode == PanelMode::LibraryOnly {
        Rect {
            x: right_area.x,
            y: area.y + tab_h,
            width: right_area.width,
            height: PLAYER_BOX_HEIGHT,
        }
    } else {
        Rect::default()
    };

    // The floating status bar band sits at the bottom of the right panel
    // only: one gap row, the status row, one padding row below it. The
    // panel paints the row inset inside the band (`status_bar_row`), so
    // the gap row, gutters, and padding row keep the library column's
    // backdrop.
    let status_area = Rect {
        x: right_area.x,
        y: right_area.y + right_area.height,
        width: right_area.width,
        height: STATUS_BAR_BAND_HEIGHT,
    };

    // Tab bar at the very top of the right column.
    let tab_bar_area = Rect {
        x: right_area.x,
        y: area.y,
        width: right_area.width,
        height: tab_h,
    };

    // The library panel's placement: the right column between the tab bar
    // and the status row. In LibraryOnly it starts below the playback
    // strip's band (placed as `library_playback`); in a queue-visible layout
    // the band is not reserved (task 4.1), so the library starts at the tab
    // bar's bottom edge. Either way the right column's present placements --
    // tab, library, status bar -- tile it with no unowned rows.
    let library_area = right_area;

    // Root panel placements (D1, task 1.3): which panels the current Panel
    // mode mounts, and where. The queue column splits into the Queue playback
    // panel's region (header row plus the visual slot/transport rows and the
    // separator row between the slot/transport band and the Queue panel) and
    // the Queue panel below it; while idle only the header row is reserved,
    // the panel's recessed inset being the single space row. Both placements
    // come from the shared `queue_panel_geometry` (task 3.2: the header row is
    // one input alongside the visual-slot and transport heights, single
    // source), so they tile the queue column's content exactly.
    let queue_col_visible = input.panel_mode != PanelMode::LibraryOnly;
    let playback_rows = queue_playback_rows(
        queue_playback_column_wide(left_area.width),
        input.card_height,
        input.playback_active,
        input.queue_title_expanded,
    );
    let queue_geo = queue_panel_geometry(QueuePanelInputs {
        left_content: left_area,
        header_height: QUEUE_PLAYBACK_HEADER_ROWS,
        card_height: playback_rows,
    });
    let queue_playback_area = Rect {
        x: left_area.x,
        y: left_area.y,
        width: left_area.width,
        height: queue_geo.panel_area.y.saturating_sub(left_area.y),
    };
    // The boundary is a hit region over the queue panel's rightmost column,
    // not a separate divider placement. The queue panels therefore paint
    // beneath it across the full queue column.
    let queue_boundary_area = Rect {
        x: area.x.saturating_add(left_w).saturating_sub(1),
        y: area.y,
        width: 1,
        height: area.height,
    };
    let tab = placed_when(right_visible, tab_bar_area);
    let library = placed_when(right_visible, library_area);
    let library_playback = placed_when(input.panel_mode == PanelMode::LibraryOnly, player_area);
    let queue = placed_when(queue_col_visible, queue_geo.panel_area);
    let queue_playback = placed_when(
        queue_col_visible && queue_geo.panel_area.height > 0,
        queue_playback_area,
    );
    let status_bar = placed_when(right_visible, status_area);
    let queue_boundary = placed_when(
        input.panel_mode == PanelMode::Both && left_w > 0,
        queue_boundary_area,
    );
    let root = RootFrame {
        tab,
        library,
        library_playback,
        queue,
        queue_playback,
        status_bar,
        queue_boundary,
    };

    FrameChromeGeometry {
        panel_area,
        left_area,
        right_area,
        left_content,
        tab_bar_area,
        right_visible,
        #[cfg(test)]
        player_area,
        #[cfg(test)]
        status_area,
        #[cfg(test)]
        queue_focused,
        root,
    }
}

#[cfg(test)]
mod root_frame_tests {
    use super::*;
    use crate::app::PanelFocus;

    fn area() -> Rect {
        Rect::new(0, 0, 100, 30)
    }

    fn frame(panel_mode: PanelMode, playback_active: bool) -> RootFrame {
        chrome_geometry(ChromeGeometryInput {
            area: area(),
            panel_mode,
            panel_focus: PanelFocus::Library,
            queue_column_width: 45,
            terminal_width: area().width,
            card_height: 12,
            playback_active,
            queue_title_expanded: false,
        })
        .root
    }

    fn placements(frame: &RootFrame) -> Vec<(&'static str, Option<Rect>)> {
        vec![
            ("tab", frame.tab),
            ("library", frame.library),
            ("library_playback", frame.library_playback),
            ("queue", frame.queue),
            ("queue_playback", frame.queue_playback),
            ("status_bar", frame.status_bar),
            ("queue_boundary", frame.queue_boundary),
        ]
    }

    /// D1's placement invariants, checked for every mode: every present
    /// placement is non-empty ("never an empty rect"), lives inside the
    /// terminal, and no two present placements overlap.
    fn assert_relational(frame: &RootFrame) {
        let present: Vec<(&'static str, Rect)> = placements(frame)
            .into_iter()
            .filter_map(|(name, rect)| rect.map(|rect| (name, rect)))
            .collect();
        let a = area();
        for (name, rect) in &present {
            assert!(
                rect.width > 0 && rect.height > 0,
                "{name} must never be an empty placement"
            );
            assert!(
                rect.x >= a.x
                    && rect.y >= a.y
                    && rect.right() <= a.right()
                    && rect.bottom() <= a.bottom(),
                "{name} must live inside the terminal"
            );
        }
        for (i, (an, ar)) in present.iter().enumerate() {
            for (bn, br) in present.iter().skip(i + 1) {
                // The resize boundary is an intentionally overlapping hit
                // region over the queue panel's rightmost column.
                if an == &"queue_boundary" || bn == &"queue_boundary" {
                    continue;
                }
                assert!(!ar.intersects(*br), "{an} overlaps {bn}");
            }
        }
    }

    /// The absent set per mode matches D1's mount rule: Tab/Library/Status bar
    /// only when the library column is visible; Library playback only when the
    /// queue column is hidden; Queue and Queue playback only when the queue
    /// column is visible; Queue boundary only in the two-panel layout. (The
    /// Queue playback placement keeps its always-painted header row when
    /// playback idles, per D10.)
    #[test]
    fn absent_set_per_mode_matches_the_mount_rule() {
        for active in [true, false] {
            let both = frame(PanelMode::Both, active);
            let present: Vec<_> = placements(&both)
                .into_iter()
                .filter(|(_, rect)| rect.is_some())
                .map(|(name, _)| name)
                .collect();
            assert_eq!(
                present,
                [
                    "tab",
                    "library",
                    "queue",
                    "queue_playback",
                    "status_bar",
                    "queue_boundary"
                ],
                "Both places every panel but the Library playback strip"
            );
            assert!(both.library_playback.is_none());

            let queue_only = frame(PanelMode::QueueOnly, active);
            assert!(queue_only.tab.is_none());
            assert!(queue_only.library.is_none());
            assert!(queue_only.library_playback.is_none());
            assert!(queue_only.status_bar.is_none());
            assert!(queue_only.queue_boundary.is_none());
            assert!(queue_only.queue.is_some());
            assert!(queue_only.queue_playback.is_some());

            let library_only = frame(PanelMode::LibraryOnly, active);
            assert!(library_only.queue.is_none());
            assert!(library_only.queue_playback.is_none());
            assert!(library_only.queue_boundary.is_none());
            assert!(library_only.tab.is_some());
            assert!(library_only.library.is_some());
            assert!(library_only.library_playback.is_some());
            assert!(library_only.status_bar.is_some());
        }
        for mode in [
            PanelMode::Both,
            PanelMode::QueueOnly,
            PanelMode::LibraryOnly,
        ] {
            assert_relational(&frame(mode, true));
            assert_relational(&frame(mode, false));
        }
    }

    /// Library-only: the four placed panels tile the library column exactly —
    /// tab bar, playback strip, library content, status row — sharing one
    /// column geometry and stacking contiguously over the full height.
    #[test]
    fn library_only_placements_tile_the_library_column() {
        let f = frame(PanelMode::LibraryOnly, true);
        let tab = f.tab.expect("tab placed");
        let strip = f.library_playback.expect("playback strip placed");
        let library = f.library.expect("library placed");
        let status = f.status_bar.expect("status bar placed");
        for rect in [tab, strip, library, status] {
            assert_eq!((rect.x, rect.width), (library.x, library.width));
        }
        assert_eq!(tab.y, area().y);
        assert_eq!(tab.bottom(), strip.y);
        assert_eq!(strip.bottom(), library.y);
        assert_eq!(library.bottom(), status.y);
        assert_eq!(status.bottom(), area().bottom());
        // The strip is exactly the shared transport arrangement's four-row
        // band (task 4.1, D10).
        assert_eq!(strip.height, PLAYER_BOX_HEIGHT);
    }

    /// Both: the queue column's two placements tile it exactly (playback
    /// region above, queue panel below, sharing one column geometry), and the
    /// right column's placed panels -- tab, library, status bar -- tile it
    /// exactly from the tab bar's bottom edge down: the strip's rows are
    /// reserved only where the strip paints (task 4.1), so the library
    /// reclaims the band in a queue-visible layout.
    #[test]
    fn both_placements_tile_the_queue_column_and_bound_the_columns() {
        let f = frame(PanelMode::Both, true);
        let queue_playback = f.queue_playback.expect("queue playback placed");
        let queue = f.queue.expect("queue placed");
        let tab = f.tab.expect("tab placed");
        let library = f.library.expect("library placed");
        let status = f.status_bar.expect("status bar placed");
        assert_eq!(
            (queue_playback.x, queue_playback.width),
            (queue.x, queue.width)
        );
        assert_eq!(queue_playback.y, area().y);
        assert_eq!(queue_playback.bottom(), queue.y);
        assert_eq!(queue.bottom(), area().bottom());
        assert_eq!(tab.y, area().y);
        // Partition property: tab, library and status bar tile the right
        // column with no unowned band between them (the library covers the
        // strip rows its panel does not place in Both mode).
        for rect in [tab, library, status] {
            assert_eq!((rect.x, rect.width), (library.x, library.width));
        }
        assert_eq!(tab.bottom(), library.y);
        assert_eq!(library.bottom(), status.y);
        assert_eq!(status.bottom(), area().bottom());
        assert!(f.library_playback.is_none());
        // No strip rows are reserved: the library spans the whole tab-to-status
        // band with no gap for the strip it does not place (task 4.1).
        assert_eq!(library.height, tab.bottom().abs_diff(status.y));
    }

    /// Queue-only: the queue column spans the full window, its two placements
    /// tile it exactly, and no right-column panel or boundary is placed.
    #[test]
    fn queue_only_placements_tile_the_full_width_queue_column() {
        let f = frame(PanelMode::QueueOnly, true);
        let queue_playback = f.queue_playback.expect("queue playback placed");
        let queue = f.queue.expect("queue placed");
        assert_eq!(
            (queue_playback.x, queue_playback.width),
            (queue.x, queue.width)
        );
        assert_eq!(queue_playback.y, area().y);
        assert_eq!(queue_playback.bottom(), queue.y);
        assert_eq!(queue.bottom(), area().bottom());
        assert_eq!(queue.width, area().width);
        assert!(f.tab.is_none());
        assert!(f.library.is_none());
        assert!(f.library_playback.is_none());
        assert!(f.status_bar.is_none());
        assert!(f.queue_boundary.is_none());
    }

    /// Idle playback collapses the visual slot/transport rows — and with them
    /// the separator row — so the Queue playback placement is exactly its
    /// always-painted header row (D10: the header is painted in every
    /// queue-visible layout, idle included); the Queue panel starts directly
    /// below it, its recessed inset providing the single space row.
    #[test]
    fn idle_queue_playback_placement_is_the_header_row() {
        let f = frame(PanelMode::Both, false);
        let queue_playback = f.queue_playback.expect("queue playback placed");
        let queue = f.queue.expect("queue placed");
        assert_eq!(
            queue_playback.height, QUEUE_PLAYBACK_HEADER_ROWS,
            "idle placement is exactly the always-painted header row"
        );
        assert_eq!(queue_playback.bottom(), queue.y);
        assert_eq!(queue_playback.height + queue.height, area().height);
    }
}
