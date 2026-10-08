use super::*;
use crate::arrangements::chrome::QUEUE_PLAYBACK_HEADER_ROWS;

/// The header band's rows: the header text row sits one `queue_panel_inset`
/// row below the column's top edge, and the queue panel starts below the
/// playback rows plus the band's gap row (the slot/transport paint here).
#[test]
fn header_rows_are_always_reserved_above_the_queue_panel() {
    let left_content = Rect::new(0, 0, 40, 30);
    let geo = queue_panel_geometry(QueuePanelInputs {
        left_content,
        header_height: QUEUE_PLAYBACK_HEADER_ROWS,
        card_height: 4,
    });
    assert_eq!(
        geo.panel_area.y,
        left_content.y + QUEUE_PLAYBACK_HEADER_ROWS + 4 + 1,
        "the header band, the playback rows, and the gap row precede the queue panel"
    );
    assert_eq!(
        geo.panel_area.height,
        left_content.height - QUEUE_PLAYBACK_HEADER_ROWS - 4 - 1
    );
}

/// The footer band's gap row: the recessed queue-list box ends one row above
/// the footer status row, leaving a blank `QueueColumn` row between the two
/// panels (and the placement's own bottom padding row below the footer).
#[test]
fn queue_list_box_ends_above_the_footer_gap_row() {
    let placement = Rect::new(0, 0, 40, 20);
    let footer = queue_footer_row(placement).expect("the footer fits a 20-row placement");
    let box_area = queue_list_box(placement);
    assert_eq!(
        box_area.bottom(),
        footer.y - 1,
        "one blank row separates the list box from the footer row"
    );
}

/// The list's rows sit inside the recessed box's one-row top pad and
/// one-row bottom spacer: both rows keep the box's own `QueuePanel` fill
/// (the frame painter fills the whole box), so the rows never touch the
/// box's edges. A degenerate box reserves everything.
#[test]
fn queue_list_rows_keep_the_box_top_pad_and_bottom_spacer() {
    let placement = Rect::new(0, 0, 40, 20);
    let box_area = queue_list_box(placement);
    let rows = queue_list_rows(box_area);
    assert_eq!(
        rows.y,
        box_area.y + 1,
        "one pad row stays above the rows inside the box"
    );
    assert_eq!(
        rows.bottom(),
        box_area.bottom() - 1,
        "one spacer row stays below the rows inside the box"
    );
    assert_eq!(rows.width, box_area.width, "the pad rows are full-width");
    let degenerate = queue_list_rows(Rect::new(0, 0, 40, 2));
    assert_eq!(
        degenerate,
        Rect::new(0, 0, 40, 2),
        "a degenerate box keeps every row"
    );
}

/// The empty-queue box centers the one-row placeholder (plus its one-row
/// top/bottom and two-column side padding) inside the list content area,
/// and shrinks to the content area when the panel is too small to fit it.
#[test]
fn empty_queue_box_centers_the_placeholder_with_its_padding() {
    // `¯\_(ツ)_/¯` is 10 columns wide, so the box is 14 wide and 3 high.
    let content = Rect::new(0, 0, 40, 20);
    let shrug = empty_queue_box(content, 10);
    assert_eq!(
        shrug,
        Rect::new((40 - 14) / 2, (20 - 3) / 2, 14, 3),
        "the box centers horizontally and vertically with its padding"
    );
    let tiny = empty_queue_box(Rect::new(5, 5, 6, 2), 10);
    assert_eq!(
        tiny,
        Rect::new(5, 5, 6, 2),
        "a content area smaller than the box keeps every row and column"
    );
}

/// The gap row between the playback band and the queue panel rides only
/// with the slot/transport rows: while they paint the queue panel starts
/// one row below them (the gap row is the playback placement's last row,
/// the shell's `QueueColumn` fill), while idle the panel borders the header
/// band directly and the box's own top pad is the single space row.
#[test]
fn the_gap_row_rides_only_with_the_slot_and_transport_rows() {
    let left_content = Rect::new(0, 0, 40, 30);
    let playing = queue_panel_geometry(QueuePanelInputs {
        left_content,
        header_height: QUEUE_PLAYBACK_HEADER_ROWS,
        card_height: 4,
    });
    assert_eq!(
        playing.panel_area.y,
        left_content.y + QUEUE_PLAYBACK_HEADER_ROWS + 4 + 1,
        "one gap row separates the playback band from the queue panel"
    );
    let idle = queue_panel_geometry(QueuePanelInputs {
        left_content,
        header_height: QUEUE_PLAYBACK_HEADER_ROWS,
        card_height: 0,
    });
    assert_eq!(
        idle.panel_area.y,
        left_content.y + QUEUE_PLAYBACK_HEADER_ROWS,
        "idle borders the header band directly: no gap row"
    );
}
