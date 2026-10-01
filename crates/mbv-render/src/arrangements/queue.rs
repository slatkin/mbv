use ratatui::layout::Rect;

/// Rows the `QueueColumn` footer band spends below the recessed queue-list
/// box: one gap row, the one-row status bar, one gap row. The status bar
/// sits outside the recessed panel, as wide as the `QueueColumn` header.
pub const QUEUE_FOOTER_BAND: u16 = 3;

#[must_use]
pub fn normalize_queue_column_width(width: u16, terminal_width: u16) -> u16 {
    use crate::layout::LEFT_WIDTH_DEFAULT;

    width.clamp(
        LEFT_WIDTH_DEFAULT,
        LEFT_WIDTH_DEFAULT.max(terminal_width.saturating_mul(3) / 5),
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QueuePanelGeometry {
    pub panel_area: Rect,
    pub content_area: Rect,
    pub footer_row: Option<Rect>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QueuePanelInputs {
    pub left_content: Rect,
    /// Rows the Queue playback panel's header row always spends above the
    /// Queue panel in every queue-visible layout, idle included (design D10;
    /// task 3.2). The panel starts directly below the playback rows and
    /// borders the transport band's bottom gap row.
    pub header_height: u16,
    pub card_height: u16,
}

/// The `QueueColumn` footer row: the status bar below the recessed panel,
/// with one gap row above it and one below, as wide as the `QueueColumn`
/// header (the column's canonical content inset). Reserved when the
/// placement fits the band alongside at least a two-row recessed box;
/// otherwise the panel keeps every row and the footer collapses.
#[must_use]
pub fn queue_footer_row(placement: Rect) -> Option<Rect> {
    (placement.height >= QUEUE_FOOTER_BAND + 4).then(|| Rect {
        x: placement.x + 2,
        y: placement.y + placement.height.saturating_sub(2),
        width: placement.width.saturating_sub(4),
        height: 1,
    })
}

/// The recessed queue-list box inside the placement: the column's canonical
/// side insets, starting directly below the playback band (the two panels
/// border one another) and ending at the footer row (no gap row above the
/// status bar). When no footer fits, the box runs to the placement's bottom.
#[must_use]
pub fn queue_list_box(placement: Rect) -> Rect {
    let mut box_area = Rect {
        x: placement.x + 2,
        width: placement.width.saturating_sub(4),
        ..placement
    };
    if let Some(footer) = queue_footer_row(placement) {
        box_area.height = footer.y.saturating_sub(box_area.y);
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
#[must_use]
pub fn queue_panel_subareas(panel_box: Rect) -> Rect {
    let padding = u16::from(panel_box.height >= 3);
    Rect {
        y: panel_box.y + padding,
        height: panel_box.height.saturating_sub(padding * 2),
        ..panel_box
    }
}

/// Places the complete queue panel and its framed sub-areas.
#[must_use]
pub fn queue_panel_geometry(input: QueuePanelInputs) -> QueuePanelGeometry {
    // The header row (always present, idle included) plus the visual-slot and
    // transport rows sit directly above the panel: no separator row between
    // the playback region and the queue list.
    let playback_rows = input.header_height + input.card_height;
    let panel_area = Rect {
        y: input.left_content.y + playback_rows,
        height: input.left_content.height.saturating_sub(playback_rows),
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
