use super::*;
use crate::arrangements::chrome::QUEUE_PLAYBACK_HEADER_ROWS;

/// The header collapse reclaims the header rows for the slot/transport
/// region: with `header_height: 0` the queue panel starts
/// `QUEUE_PLAYBACK_HEADER_ROWS` rows higher and the playback region grows by
/// exactly those rows (the hidden-header rule's geometry half).
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
        header_height: 0,
        card_height: 4,
    });
    assert_eq!(
        collapsed.panel_area.y,
        with_header.panel_area.y - QUEUE_PLAYBACK_HEADER_ROWS
    );
    assert_eq!(
        collapsed.panel_area.height,
        with_header.panel_area.height + QUEUE_PLAYBACK_HEADER_ROWS
    );
}
