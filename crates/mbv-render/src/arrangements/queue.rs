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
    /// task 3.2). The panel starts one row below the playback rows while the
    /// slot/transport paint (the band's gap row); while idle it borders the
    /// header band directly.
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
/// border one another) and ending just above the gap row that precedes the
/// footer. The footer band's rows keep the `QueueColumn` surface; the row
/// below the footer is the placement's own bottom padding. When no footer
/// fits, the box runs to the placement's bottom.
#[must_use]
pub fn queue_list_box(placement: Rect) -> Rect {
    let mut box_area = Rect {
        x: placement.x + 2,
        width: placement.width.saturating_sub(4),
        ..placement
    };
    if let Some(footer) = queue_footer_row(placement) {
        // The box's last row is the one above the footer's gap row.
        box_area.height = (footer.y - 1).saturating_sub(box_area.y);
    }
    box_area
}

/// The queue list's rows inside the recessed box: the box minus its one-row
/// top pad and one-row bottom spacer. Both rows keep the box's own
/// `QueuePanel` fill (the recessed panel's surface), so the rows never touch
/// the box's edges; the frame painter fills them, the list just does not
/// paint there. A degenerate box (under three rows) reserves everything.
#[must_use]
pub fn queue_list_rows(box_area: Rect) -> Rect {
    let padding = u16::from(box_area.height >= 3);
    Rect {
        y: box_area.y + padding,
        height: box_area.height.saturating_sub(padding * 2),
        ..box_area
    }
}

/// The queue list's painted rows for one panel placement: the recessed box
/// plus its inner pad/spacer inset. The one place the panel, the shell's
/// placement projection, and the panel geometry derive the rows they paint
/// or project, so the inset cannot drift between them.
#[must_use]
pub fn queue_list_content(placement: Rect) -> Rect {
    queue_list_rows(queue_list_box(placement))
}

/// The empty-queue placeholder's box inside the list content area: the one
/// text row plus one row of padding above and below and two columns of
/// padding on each side, centered both vertically and horizontally.
/// `text_width` is the display width of the placeholder text. A content area
/// too small to fit the box keeps every row and column it has.
#[must_use]
pub fn empty_queue_box(content: Rect, text_width: u16) -> Rect {
    if content.width == 0 || content.height == 0 {
        return content;
    }
    let box_width = text_width.saturating_add(4).min(content.width);
    let box_height = 3u16.min(content.height);
    Rect {
        x: content.x + (content.width - box_width) / 2,
        y: content.y + (content.height - box_height) / 2,
        width: box_width,
        height: box_height,
    }
}

/// Places the complete queue panel and its framed sub-areas.
#[must_use]
pub fn queue_panel_geometry(input: QueuePanelInputs) -> QueuePanelGeometry {
    // The header row (always present, idle included) plus the visual-slot and
    // transport rows sit above the panel, separated from it by one gap row
    // while the slot/transport paint. The gap belongs to the slot/transport
    // band: while idle nothing renders between the header and the panel, and
    // the box's own top pad is the single space row below the header. The
    // gap row is the playback region placement's last row (the placement
    // runs to the panel's first row), so the shell's QueueColumn fill owns it.
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
    let (content_area, footer_row) = (queue_list_content(panel_area), queue_footer_row(panel_area));
    QueuePanelGeometry {
        panel_area,
        content_area,
        footer_row,
    }
}

#[cfg(test)]
mod tests;
