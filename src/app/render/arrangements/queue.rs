use ratatui::layout::Rect;

use crate::app::render::components::widgets::queue_panel_inset;

/// Rows the `QueueColumn` footer band spends below the recessed queue-list
/// box: one gap row, the one-row status bar, one gap row. The status bar
/// sits outside the recessed panel, as wide as the `QueueColumn` header.
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

/// The `QueueColumn` footer row: the status bar below the recessed panel,
/// with one gap row above it and one below, as wide as the `QueueColumn`
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
/// band's rows keep the `QueueColumn` surface; the row below the footer is the
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

/// The framed list content inside the recessed box. Shared by the root
/// placement (`queue_panel_geometry`) and the mounted `QueuePanel`'s own
/// view, which derives the same content from the placement it is handed
/// (task 3.1) -- one source for the panel's internal geometry. The list keeps
/// the box's own one-row top inset and bottom padding; there is no title band
/// above it. The status bar lives in the `QueueColumn` footer below the box. A
/// degenerate box reserves nothing.
pub(in crate::app) fn queue_panel_subareas(panel_box: Rect) -> Rect {
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
