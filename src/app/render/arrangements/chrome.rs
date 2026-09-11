use super::queue::{queue_panel_geometry, QueuePanelInputs};
use crate::app::layout::FrameChromeGeometry;
use crate::app::render::components::chrome;
use crate::app::render::components::widgets::COLUMN_GAP;
use crate::app::{PanelFocus, PanelMode, TABBAR_LEFT_RESERVE};
use ratatui::layout::Rect;

/// Height of the tab-bar box: 1 row padding + 1 row tab + 1 row spacer.
const TAB_BAR_BOX_HEIGHT: u16 = 3;

/// Height of the player panel box below the tab bar (seekbar + title +
/// controls rows).
pub(in crate::app) const PLAYER_BOX_HEIGHT: u16 = 4;

/// Rows the Queue playback panel always spends on its header row in every
/// queue-visible layout, idle included (design D10; the painted row lands
/// with task 3.2). The root placement reserves it now so the Queue panel's
/// placement already starts below the Queue playback panel's header.
pub(in crate::app) const QUEUE_PLAYBACK_HEADER_ROWS: u16 = 1;

/// Resolved app state needed to place the frame chrome for one terminal area.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct ChromeGeometryInput {
    pub area: Rect,
    pub panel_mode: PanelMode,
    pub panel_focus: PanelFocus,
    pub queue_column_width: u16,
    pub terminal_width: u16,
    /// The queue card/visual-slot height as published by the last full frame
    /// (`AppLayout::main.card`). The card's authoritative size is paint-coupled
    /// until the visual slot moves into the Queue playback panel (tasks
    /// 3.4/3.5); the root placements and the queue geometry share the row
    /// count through `queue_playback_rows`, though the two still place the
    /// queue panel at different offsets (the placement also spends the
    /// playback header row) until S2 consumes the placement.
    pub card_height: u16,
    /// Whether playback is active (the idle collapse collapses the card to
    /// zero rows).
    pub playback_active: bool,
    /// Whether a connected remote/cast transport keeps the queue-only
    /// playback panel visible while playback is idle.
    pub transport_connected: bool,
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RootFrame {
    /// Tab bar at the top of the library column (`TabPanel`, task 2.1).
    pub tab: Option<Rect>,
    /// The library column's content area (below the tab bar; below the
    /// playback strip in LibraryOnly) (`LibraryPanel`, task 4.1). In Both the
    /// strip panel is not placed but its rows are still reserved, so this
    /// placement covers the strip band from the tab bar down — S0
    /// intermediate; task 4.1 stops reserving the strip rows and re-derives
    /// this.
    pub library: Option<Rect>,
    /// The right-column playback strip (`LibraryPlaybackPanel`, task 4.1):
    /// placed only when the queue column is hidden.
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

/// A panel placement that exists only when its mount-rule condition holds and
/// the computed rect is paintable: a panel absent from a mode has no
/// placement, never an empty rect (design D1's live-ICF mount rule).
fn placed_when(cond: bool, rect: Rect) -> Option<Rect> {
    cond.then_some(rect).filter(|r| r.width > 0 && r.height > 0)
}

/// Rows the queue column spends above the queue panel: the card/visual-slot
/// rows plus, in queue-only layouts, the queue-only transport rows (stacked on
/// narrow terminals, beside the card at 100+ columns, where the taller of the
/// two governs). Shared by `render_main`'s authoritative frame computation
/// and the root placements so both spend the same playback rows; the two
/// still place the queue panel at different offsets (the placement also
/// spends the playback header row) until S2 consumes the placement. Idle
/// collapse collapses the card to zero rows; a connected transport keeps
/// its queue-only panel.
pub(in crate::app) fn queue_playback_rows(
    queue_only: bool,
    queue_only_wide: bool,
    card_height: u16,
    playback_active: bool,
    transport_connected: bool,
) -> u16 {
    let card = if playback_active { card_height } else { 0 };
    if queue_only && (playback_active || transport_connected) {
        if queue_only_wide {
            card.max(PLAYER_BOX_HEIGHT)
        } else {
            card.saturating_add(PLAYER_BOX_HEIGHT)
        }
    } else {
        card
    }
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
    let panel_area = if input.terminal_width < crate::app::MINI_VIEW_THRESHOLD
        && input.panel_mode == PanelMode::LibraryOnly
    {
        area
    } else {
        left_area
    };
    let panel_content_area = chrome::left_panel_content_area(panel_area);
    let queue_focused = matches!(input.panel_focus, PanelFocus::Queue);

    // Full-column background behind the card image and queue list.
    let right_full_area = Rect {
        x: area.x + left_w + COLUMN_GAP,
        y: area.y,
        width: right_w.saturating_sub(COLUMN_GAP),
        height: area.height,
    };

    // Inner content area with padding inside the colored box (queue uses this).
    let left_content = Rect {
        x: left_area.x + 2,
        y: left_area.y + 1,
        width: left_area.width.saturating_sub(4),
        height: left_area.height.saturating_sub(2),
    };

    let tab_h: u16 = TAB_BAR_BOX_HEIGHT;
    let right_area = Rect {
        x: area.x + left_w + COLUMN_GAP,
        y: area.y + tab_h + PLAYER_BOX_HEIGHT,
        width: right_w.saturating_sub(COLUMN_GAP),
        height: content_h
            .saturating_sub(1)
            .saturating_sub(tab_h)
            .saturating_sub(PLAYER_BOX_HEIGHT),
    };

    // Player panel below the tab bar (right column only).
    let player_area = if right_visible {
        Rect {
            x: right_area.x,
            y: area.y + tab_h,
            width: right_area.width,
            height: PLAYER_BOX_HEIGHT,
        }
    } else {
        Rect::default()
    };

    // Status bar sits at the bottom of the right panel only.
    let status_area = Rect {
        x: right_area.x,
        y: right_area.y + right_area.height,
        width: right_area.width,
        height: 1,
    };

    // Tab bar at the very top of the right column.
    let tab_bar_area = Rect {
        x: right_area.x,
        y: area.y,
        width: right_area.width,
        height: tab_h,
    };

    // The library panel's placement. In LibraryOnly the playback strip is
    // placed as `library_playback`, so the library starts below it. In Both
    // the strip panel is not placed (D1: it mounts only when the queue column
    // is hidden), but the legacy strip rows are still reserved in the right
    // column; the library placement covers that band so the right column's
    // present placements -- tab, library, status bar -- tile it with no
    // unowned rows (S0 intermediate; task 4.1 stops reserving the strip rows).
    let library_area = if input.panel_mode == PanelMode::LibraryOnly {
        right_area
    } else {
        Rect {
            y: tab_bar_area.bottom(),
            height: right_area.bottom().saturating_sub(tab_bar_area.bottom()),
            ..right_area
        }
    };

    // Root panel placements (D1, task 1.3): which panels the current Panel
    // mode mounts, and where. The queue column splits into the Queue playback
    // panel's region (header row + visual slot/transport rows plus the
    // separating gap row) and the Queue panel below it. Both placements come
    // from the shared `queue_panel_geometry` (task 3.2: the header row is one
    // input alongside the visual-slot and transport heights, single source),
    // so they tile the queue column's content exactly.
    let queue_col_visible = input.panel_mode != PanelMode::LibraryOnly;
    let playback_rows = queue_playback_rows(
        input.panel_mode == PanelMode::QueueOnly,
        input.panel_mode == PanelMode::QueueOnly && left_area.width >= 100,
        input.card_height,
        input.playback_active,
        input.transport_connected,
    );
    let queue_geo = queue_panel_geometry(QueuePanelInputs {
        left_content,
        header_height: QUEUE_PLAYBACK_HEADER_ROWS,
        card_height: playback_rows,
        narrow_player_height: 0,
    });
    let queue_playback_area = Rect {
        x: left_content.x,
        y: left_content.y,
        width: left_content.width,
        height: queue_geo.panel_area.y.saturating_sub(left_content.y),
    };
    let queue_boundary_area = Rect {
        x: area.x.saturating_add(left_w).saturating_sub(1),
        y: area.y,
        width: 1,
        height: area.height,
    };
    let root = RootFrame {
        tab: placed_when(right_visible, tab_bar_area),
        library: placed_when(right_visible, library_area),
        library_playback: placed_when(input.panel_mode == PanelMode::LibraryOnly, player_area),
        queue: placed_when(queue_col_visible, queue_geo.panel_area),
        queue_playback: placed_when(
            queue_col_visible && queue_geo.panel_area.height > 0,
            queue_playback_area,
        ),
        status_bar: placed_when(right_visible, status_area),
        queue_boundary: placed_when(
            input.panel_mode == PanelMode::Both && left_w > 0,
            queue_boundary_area,
        ),
    };

    FrameChromeGeometry {
        panel_area,
        panel_content_area,
        left_area,
        right_area,
        right_full_area,
        left_content,
        tab_bar_area,
        player_area,
        status_area,
        right_visible,
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
            transport_connected: false,
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
    }

    /// Both: the queue column's two placements tile it exactly (playback
    /// region above, queue panel below, sharing one column geometry), the
    /// boundary is the one-column divider beside the queue column, and the
    /// right column's placed panels -- tab, library, status bar -- tile it
    /// exactly: the library starts at the tab bar's bottom edge and covers
    /// the legacy strip band (whose panel mounts only when the queue column
    /// is hidden — task 4.1 stops reserving those rows).
    #[test]
    fn both_placements_tile_the_queue_column_and_bound_the_columns() {
        let f = frame(PanelMode::Both, true);
        let queue_playback = f.queue_playback.expect("queue playback placed");
        let queue = f.queue.expect("queue placed");
        let tab = f.tab.expect("tab placed");
        let library = f.library.expect("library placed");
        let status = f.status_bar.expect("status bar placed");
        let boundary = f.queue_boundary.expect("boundary placed");

        assert_eq!(
            (queue_playback.x, queue_playback.width),
            (queue.x, queue.width)
        );
        assert_eq!(queue_playback.y, area().y + 1);
        assert_eq!(queue_playback.bottom(), queue.y);
        assert_eq!(queue.bottom(), area().bottom() - 1);
        assert!(queue.right() <= boundary.x);

        assert_eq!(boundary.width, 1);
        assert_eq!(boundary.height, area().height);
        assert_eq!(boundary.x + 1 + COLUMN_GAP, tab.x);
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
        assert_eq!(queue_playback.y, area().y + 1);
        assert_eq!(queue_playback.bottom(), queue.y);
        assert_eq!(queue.bottom(), area().bottom() - 1);
        assert_eq!(queue.width, area().width - 4);
        assert!(f.tab.is_none());
        assert!(f.library.is_none());
        assert!(f.library_playback.is_none());
        assert!(f.status_bar.is_none());
        assert!(f.queue_boundary.is_none());
    }

    /// Idle playback collapses the visual slot/transport rows to zero, so the
    /// Queue playback placement is its header row plus the single separator
    /// row above the Queue panel (D10: the header is painted in every
    /// queue-visible layout, idle included); the Queue panel starts directly
    /// below it.
    #[test]
    fn idle_queue_playback_placement_is_the_header_row() {
        let f = frame(PanelMode::Both, false);
        let queue_playback = f.queue_playback.expect("queue playback placed");
        let queue = f.queue.expect("queue placed");
        assert_eq!(
            queue_playback.height,
            QUEUE_PLAYBACK_HEADER_ROWS + 1,
            "idle placement is the always-painted header row plus the separator row"
        );
        assert_eq!(queue_playback.bottom(), queue.y);
        assert_eq!(
            queue_playback.height + queue.height,
            area().height.saturating_sub(2)
        );
    }
}
