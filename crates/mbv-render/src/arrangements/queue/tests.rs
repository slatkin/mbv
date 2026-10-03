use super::*;
use crate::arrangements::chrome::{QUEUE_PLAYBACK_HEADER_ROWS, queue_playback_header_rows};

/// The header collapse reclaims the header text row for the slot/transport
/// region: with `header_height: QUEUE_PLAYBACK_HEADER_ROWS - 1` (the recess
/// row stays as the blank row above the image) the queue panel starts one
/// row higher and the playback region grows by exactly that row (the
/// hidden-header rule's geometry half).
#[test]
fn zero_header_height_reclaims_the_header_rows() {
    let left_content = Rect::new(0, 0, 40, 30);
    let with_header = queue_panel_geometry(QueuePanelInputs {
        left_content,
        header_height: QUEUE_PLAYBACK_HEADER_ROWS,
        card_height: 4,
    });
    let collapsed = queue_panel_geometry(QueuePanelInputs {
        left_content,
        header_height: queue_playback_header_rows(false),
        card_height: 4,
    });
    assert_eq!(
        collapsed.panel_area.y,
        with_header.panel_area.y - 1,
        "only the header text row is reclaimed; the recess row stays"
    );
    assert_eq!(
        collapsed.panel_area.height,
        with_header.panel_area.height + 1
    );
}
