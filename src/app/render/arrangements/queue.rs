use ratatui::layout::Rect;

use crate::app::render::components::widgets::queue_panel_inset;

/// Rows the QueueColumn footer band spends below the recessed queue-list
/// box: one gap row, the one-row status bar, one gap row. The status bar
/// sits outside the recessed panel, as wide as the QueueColumn header.
pub(in crate::app) const QUEUE_FOOTER_BAND: u16 = 3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct QueuePanelGeometry {
    pub panel_area: Rect,
    pub content_area: Rect,
    pub footer_row: Option<Rect>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct QueuePanelInputs {
    pub left_content: Rect,
    /// Rows the Queue playback panel's header row always spends above the
    /// Queue panel in every queue-visible layout, idle included (design D10;
    /// task 3.2). While idle the header row is the only space above the
    /// Queue panel besides the panel's own recessed inset; the separator row
    /// is reserved only alongside the visual-slot and transport heights.
    pub header_height: u16,
    pub card_height: u16,
}

/// The QueueColumn footer row: the status bar below the recessed panel,
/// with one gap row above it and one below, as wide as the QueueColumn
/// header (the column's canonical content inset). Reserved when the
/// placement fits the band alongside at least a two-row recessed box;
/// otherwise the panel keeps every row and the footer collapses.
pub(in crate::app) fn queue_footer_row(placement: Rect) -> Option<Rect> {
    (placement.height >= QUEUE_FOOTER_BAND + 4).then(|| Rect {
        x: placement.x + 2,
        y: placement.y + placement.height.saturating_sub(2),
        width: placement.width.saturating_sub(4),
        height: 1,
    })
}

/// The recessed queue-list box inside the placement: the column's canonical
/// inset, ended just above the gap row that precedes the footer. The footer
/// band's rows keep the QueueColumn surface; the row below the footer is the
/// placement's own bottom padding.
pub(in crate::app) fn queue_list_box(placement: Rect) -> Rect {
    let mut box_area = queue_panel_inset(placement);
    if let Some(footer) = queue_footer_row(placement) {
        // The box's last row is the one above the footer's gap row: the
        // recessed panel keeps its own bottom padding row there.
        box_area.height = (footer.y - 1).saturating_sub(box_area.y);
    }
    box_area
}

/// The `Queue` title row inside the recessed box: one blank top-inset row
/// stays above it, the list starts directly below it. Reserved only when
/// the box fits padding + title + at least one list row + bottom padding;
/// smaller boxes keep the legacy title-less content.
pub(in crate::app) fn queue_panel_title_row(panel_box: Rect) -> Option<Rect> {
    (panel_box.height >= 4 && panel_box.width > 0).then(|| Rect {
        x: panel_box.x,
        y: panel_box.y + 1,
        width: panel_box.width,
        height: 1,
    })
}

/// The framed list content inside the recessed box. Shared by the root
/// placement (`queue_panel_geometry`) and the mounted `QueuePanel`'s own
/// view, which derives the same content from the placement it is handed
/// (task 3.1) -- one source for the panel's internal geometry. With a title
/// row present the list starts directly below the title and ends above the
/// box's own blank bottom-padding row (one blank top-inset row stays above
/// the title); without one (degenerate boxes) the list keeps the legacy
/// top/bottom inset, and the status bar lives in the QueueColumn footer
/// below the box. A degenerate box reserves nothing.
pub(in crate::app) fn queue_panel_subareas(panel_box: Rect) -> Rect {
    if queue_panel_title_row(panel_box).is_some() {
        return Rect {
            y: panel_box.y + 2,
            height: panel_box.height.saturating_sub(3),
            ..panel_box
        };
    }
    let padding = u16::from(panel_box.height >= 3);
    Rect {
        y: panel_box.y + padding,
        height: panel_box.height.saturating_sub(padding * 2),
        ..panel_box
    }
}

/// Places the complete queue panel and its framed sub-areas.
pub(in crate::app) fn queue_panel_geometry(input: QueuePanelInputs) -> QueuePanelGeometry {
    // The header row (always present, idle included) plus the visual-slot and
    // transport rows sit above the panel, separated from it by one gap row.
    // The gap belongs to the slot/transport band: while idle nothing renders
    // between the header and the panel, and the panel's recessed top inset is
    // the single space row below the header.
    let playback_rows = input.header_height + input.card_height;
    let gap = u16::from(input.card_height > 0);
    let panel_area = Rect {
        y: input.left_content.y + playback_rows + gap,
        height: input
            .left_content
            .height
            .saturating_sub(playback_rows)
            .saturating_sub(gap),
        ..input.left_content
    };
    let (content_area, footer_row) = (
        queue_panel_subareas(queue_list_box(panel_area)),
        queue_footer_row(panel_area),
    );
    QueuePanelGeometry {
        panel_area,
        content_area,
        footer_row,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn left_content(height: u16) -> Rect {
        Rect::new(0, 0, 40, height)
    }

    /// Task 3.2: the Queue panel always starts below the Queue playback
    /// panel's placement (header row + visual slot/transport rows), with one
    /// separator row whenever slot/transport rows exist. While idle only the
    /// header row is reserved above the panel (the panel's recessed inset is
    /// the single space row).
    #[test]
    fn queue_panel_starts_below_header_and_separator_in_every_playback_state() {
        let header = 1u16;
        // (card_rows, separator_rows, label)
        let cases: [(u16, u16, &str); 3] = [
            (0, 0, "idle: no visual slot, no transport, no separator"),
            (
                2,
                1,
                "paused: visual slot only (transport rows collapse into the slot height)",
            ),
            (6, 1, "playing: visual slot + transport rows"),
        ];
        for (card_rows, gap, label) in cases {
            let geometry = queue_panel_geometry(QueuePanelInputs {
                left_content: left_content(30),
                header_height: header,
                card_height: card_rows,
            });
            // Header row (always) + card/transport rows + the separator row.
            assert_eq!(
                geometry.panel_area.y,
                header + card_rows + gap,
                "{label}: the queue panel must start below the header row and its separator"
            );
            assert_eq!(
                geometry.panel_area.height,
                30 - header - card_rows - gap,
                "{label}: the queue panel takes the remaining rows"
            );
            assert_eq!(
                geometry.panel_area.bottom(),
                30,
                "{label}: the queue panel ends at the column's content bottom"
            );
            assert!(
                geometry.content_area.y >= geometry.panel_area.y,
                "{label}: sub-areas live inside the panel"
            );
        }
    }

    /// The list content sits below the title row: one blank top-inset row,
    /// the one-row title, the list, one blank bottom-padding row (no
    /// in-panel status reservation: the status bar lives in the
    /// QueueColumn footer below the box).
    #[test]
    fn queue_panel_subareas_stay_inside_the_box() {
        let panel_box = Rect::new(2, 6, 30, 20);
        let title = queue_panel_title_row(panel_box).expect("roomy box reserves a title");
        assert_eq!((title.x, title.y, title.width, title.height), (2, 7, 30, 1));
        let content = queue_panel_subareas(panel_box);
        assert!(panel_box.contains((content.x, content.y).into()));
        assert_eq!(content.y, title.y + 1);
        assert_eq!(content.bottom(), panel_box.bottom() - 1);
        assert_eq!(content.height, panel_box.height - 3);

        // A title-less box keeps the legacy single inset (no title band).
        let content = queue_panel_subareas(Rect::new(0, 0, 10, 3));
        assert_eq!((content.y, content.height), (1, 1));
        assert!(queue_panel_title_row(Rect::new(0, 0, 10, 3)).is_none());

        // A degenerate box reserves nothing.
        let content = queue_panel_subareas(Rect::new(0, 0, 10, 2));
        assert_eq!(content.height, 2);
    }

    /// The footer sits outside the recessed box: one gap row above it, one
    /// below, as wide as the box (the QueueColumn header width). The recessed
    /// box keeps its own blank bottom-padding row above the gap.
    #[test]
    fn footer_sits_below_the_recessed_box_with_gaps() {
        let placement = Rect::new(0, 5, 40, 20);
        let footer = queue_footer_row(placement).expect("20-row placement reserves a footer");
        let panel_box = queue_list_box(placement);
        assert_eq!(
            (footer.x, footer.width),
            (placement.x + 2, placement.width - 4)
        );
        assert_eq!(footer.height, 1);
        assert_eq!(
            panel_box.bottom(),
            footer.y - 1,
            "exactly one gap row sits between the recessed box and the footer"
        );
        assert_eq!(footer.bottom(), placement.bottom() - 1);
        assert_eq!((panel_box.x, panel_box.width), (footer.x, footer.width));
        // The box's last row is its own bottom padding, below the content.
        let content = queue_panel_subareas(panel_box);
        assert_eq!(content.bottom(), panel_box.bottom() - 1);
        assert!(content.bottom() < panel_box.bottom());

        // A short placement keeps every row for the panel: no footer.
        let small = Rect::new(0, 0, 40, 6);
        assert!(queue_footer_row(small).is_none());
        assert_eq!(queue_list_box(small), queue_panel_inset(small));
    }

    /// The idle queue-only pane-geometry chain (review of tasks 3.1-3.4,
    /// moved from the queue render test): with no visual slot or transport
    /// rows, the list content starts below the header row plus the panel's
    /// one blank top-inset row plus the one-row title — all from the
    /// arrangement's own inputs, not pulled from a render-test buffer.
    #[test]
    fn idle_pane_starts_below_header_with_a_title_band() {
        let header = 1u16;
        let left = left_content(30);
        let geometry = queue_panel_geometry(QueuePanelInputs {
            left_content: left,
            header_height: header,
            card_height: 0,
        });
        // Header row plus the column inset row plus the recessed box's one
        // blank top-inset row plus the one-row title; the list starts
        // directly below the title. (The shell helper now resolves the same
        // content the mounted panel paints, through the shared box helper.)
        assert_eq!(geometry.content_area.y, left.y + header + 3);
        assert_eq!(geometry.panel_area.bottom(), 30);
    }
}
